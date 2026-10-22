"""Synthetic upstream responses only. No Paddle, images, network or model required."""
import base64
from concurrent.futures import Future
from types import SimpleNamespace as N
import unittest
from paddle_vl_receipts import ReceiptCapture, digest
from paddle_ocr_vl import loopback_url, visual_lines


def installed():
    client=N(create_chat_completion=lambda *a,**k: Future())
    model=N(genai_client=client)
    inner=N(vl_rec_model=model,
            _paddleocr_vl_run_vl_recognition_batches=lambda *a: None,
            _paddleocr_vl_assemble_parsing_results=lambda *a: None)
    capture=ReceiptCapture();capture.install(N(paddlex_pipeline=N(_pipeline=inner)))
    return capture,model


class Receipts(unittest.TestCase):
    def test_actual_finish_reason_and_request_hash_are_retained(self):
        capture,model=installed()
        f=model.genai_client.create_chat_completion([dict(content=[dict(type='image_url',image_url=dict(url='data:image/png;base64,'+base64.b64encode(b'synthetic-crop').decode()))])],max_tokens=9000)
        f.set_result(N(id='synthetic',choices=[N(message=N(content='synthetic text'),finish_reason='length')],usage=N(completion_tokens=4096)))
        self.assertEqual(model._doc_vlm_genai_collect_responses([f]),['synthetic text'])
        r=capture.responses[0]
        self.assertEqual(r['finishReason'],'length')
        self.assertEqual(r['maxTokens'],4096)
        self.assertEqual(r['timeoutSeconds'],45)
        self.assertEqual(len(r['inputImageSha256']),64)

    def test_empty_response_keeps_original_null_without_inventing_text(self):
        capture,model=installed();f=Future()
        f.set_result(N(id='synthetic-empty',choices=[N(message=N(content=None),finish_reason='stop')],usage=None))
        self.assertEqual(model._doc_vlm_genai_collect_responses([f]),[''])
        self.assertEqual(capture.responses[0]['rawText'],'')
        self.assertEqual(capture.responses[0]['finishReason'],'stop')

    def test_exception_message_never_enters_receipt(self):
        capture,model=installed();f=Future();f.set_exception(RuntimeError('PRIVATE-CANARY'))
        self.assertEqual(model._doc_vlm_genai_collect_responses([f]),[''])
        self.assertEqual(capture.responses[0]['finishReason'],'error')
        self.assertNotIn('PRIVATE-CANARY',str(capture.responses))

    def test_timeout_is_error_not_stop(self):
        capture,model=installed()
        class TimedOut:
            def result(self,timeout):
                self.timeout=timeout
                raise TimeoutError('PRIVATE-CANARY')
            def cancel(self):self.cancelled=True
        f=TimedOut();model._doc_vlm_genai_collect_responses([f])
        self.assertEqual(f.timeout,46);self.assertTrue(f.cancelled)
        self.assertEqual(capture.responses[0]['finishReason'],'error')

    def test_binding_preserves_raw_and_postprocessing_hashes(self):
        capture,_=installed()
        response=dict(rawOutputSha256=digest('111111'),finishReason='length',requestRef='synthetic',generatedTokens=6)
        capture.bound=[dict(label='text',bbox=[0,0,20,20],response=response)]
        receipts=capture.for_blocks([dict(block_id=4,block_label='text',block_bbox=[0,0,20,20],block_content='11')])
        self.assertEqual(receipts[0]['rawOutputSha256'],digest('111111'))
        self.assertEqual(receipts[0]['outputSha256'],digest('11'))
        self.assertEqual(receipts[0]['finishReason'],'length')

    def test_ambiguous_geometry_is_missing_evidence(self):
        capture,_=installed();capture.bound=[dict(label='text',bbox=[0,0,20,20])]*2
        self.assertEqual(capture.for_blocks([dict(block_id=4,block_label='text',block_bbox=[0,0,20,20],block_content='same')]),[])

    def test_literal_loopback_only(self):
        self.assertEqual(loopback_url('http://127.0.0.1:18094/v1'),'http://127.0.0.1:18094/v1')
        for url in ['https://example.com','http://localhost:80','http://127.0.0.1:80/?secret=1','http://user@127.0.0.1:80']:
            with self.assertRaises(ValueError):loopback_url(url)

    def test_coloured_words_follow_visual_rows(self):
        lines=[dict(text='middle',bbox=[30,5,50,15]),dict(text='left',bbox=[0,6,20,16]),dict(text='right',bbox=[60,4,80,14])]
        self.assertEqual([x['text']for x in visual_lines(lines)],['left','middle','right'])


if __name__=='__main__':unittest.main()
