"""Version-pinned, process-local capture of actual PaddleOCR-VL region responses.

No server, queue or storage owner. The bridge calls the normal full Paddle pipeline.
Hooks retain what the 3.7.2 assembly normally discards; installed packages are not edited.
"""
from __future__ import annotations
import base64
import hashlib
import types


def digest(text: str) -> str:
    return hashlib.sha256(text.encode('utf-8')).hexdigest()


class ReceiptCapture:
    def __init__(self):
        self.responses = []
        self.bound = []
        self.inputs = {}

    def install(self, pipeline):
        inner = pipeline.paddlex_pipeline._pipeline
        model = inner.vl_rec_model
        capture = self
        original_request = model.genai_client.create_chat_completion

        def bounded_request(_client, *args, **kwargs):
            kwargs['timeout'] = 45
            kwargs['max_tokens'] = min(int(kwargs.get('max_tokens', 4096)), 4096)
            future=original_request(*args, **kwargs)
            messages=args[0] if args else kwargs.get('messages',[])
            images=[part['image_url']['url'] for message in messages for part in message.get('content',[]) if isinstance(part,dict) and part.get('type')=='image_url']
            capture.inputs[id(future)]=dict(inputImageSha256=hashlib.sha256(base64.b64decode(images[0].split(',',1)[1])).hexdigest() if len(images)==1 and images[0].startswith('data:image/') else None,
                                           timeoutSeconds=45,maxTokens=kwargs['max_tokens'])
            return future

        def collect(_model, futures):
            texts = []
            for future in futures:
                try:
                    response = future.result(timeout=46)
                    choice = response.choices[0]
                    text = choice.message.content or ''
                    usage = getattr(response, 'usage', None)
                    item = dict(requestRef=response.id, finishReason=choice.finish_reason,
                                generatedTokens=getattr(usage, 'completion_tokens', None),
                                rawText=text, rawOutputSha256=digest(text), error=None, rawResponse=response.model_dump() if hasattr(response,'model_dump') else None)
                except Exception as error:
                    future.cancel()
                    text = ''
                    # Error category only, never the request, response body or exception text.
                    item = dict(requestRef=None, finishReason='error', generatedTokens=None,
                                rawText=text, rawOutputSha256=digest(text), error=type(error).__name__)
                item.update(capture.inputs.pop(id(future),{}))
                capture.responses.append(item)
                texts.append(text)
            return texts

        original_run = inner._paddleocr_vl_run_vl_recognition_batches
        def run_batches(_inner, batches, spotting, kwargs):
            start = len(capture.responses)
            original_run(batches, spotting, kwargs)
            records = capture.responses[start:]
            cursor = 0
            for info in batches.values():
                count = len(info['vlm_results'])
                attached = records[cursor:cursor+count]
                if len(attached) != count:
                    raise ValueError('region_receipt_count_mismatch')
                for result, item in zip(info['vlm_results'], attached, strict=True):
                    if digest(result.get('result') or '') != item['rawOutputSha256']:
                        raise ValueError('region_receipt_text_mismatch')
                info['_linggan_receipts'] = attached
                cursor += count
            if cursor != len(records):
                raise ValueError('region_receipt_extra_response')

        original_assemble = inner._paddleocr_vl_assemble_parsing_results
        def assemble(_inner, blocks, batches, mapping, *args):
            for (page, index), pixel_key in mapping.items():
                info = batches[pixel_key]
                n = info['vlm_block_ids'].index((page, index))
                block = blocks[page][index]
                capture.bound.append(dict(label=block['label'], bbox=list(block['box']),
                                          response=info['_linggan_receipts'][n]))
            return original_assemble(blocks, batches, mapping, *args)

        model.genai_client.create_chat_completion = types.MethodType(bounded_request, model.genai_client)
        model._doc_vlm_genai_collect_responses = types.MethodType(collect, model)
        inner._paddleocr_vl_run_vl_recognition_batches = types.MethodType(run_batches, inner)
        inner._paddleocr_vl_assemble_parsing_results = types.MethodType(assemble, inner)

    def reset(self):
        self.inputs.clear()
        self.responses.clear()
        self.bound.clear()

    def for_blocks(self, blocks):
        receipts = []
        for block in blocks:
            found = [r for r in self.bound if r['label'] == block['block_label']
                     and list(r['bbox']) == list(block['block_bbox'])]
            if len(found) != 1:
                # Ambiguous geometry is missing evidence, not a sequential-log guess.
                continue
            response = found[0]['response']
            receipts.append(dict(blockRef=str(block['block_id']),
                                 outputSha256=digest(block['block_content']),
                                 finishReason=response['finishReason'], requestRef=response['requestRef'],
                                 rawOutputSha256=response['rawOutputSha256'],
                                 generatedTokens=response['generatedTokens']))
        return receipts
