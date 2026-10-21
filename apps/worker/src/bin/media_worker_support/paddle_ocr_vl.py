#!/usr/bin/env python3
"""One local image through full PaddleOCR-VL + retained PP-OCRv4 line observations.

Explicit model paths and a literal loopback endpoint only; no installation or scheduling.
Rust owns source identity, job leases, storage and database writes. This bridge only returns
an observation bundle. Optional crops are limited to disagreements, never an OCR rewrite.
"""
from __future__ import annotations
import hashlib
import io
import json
import math
import os
from pathlib import Path
import sys
import unicodedata
from urllib.parse import urlsplit
from paddle_vl_receipts import ReceiptCapture


def encoded(value):
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'), sort_keys=True)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def normalized(text):
    return ''.join(c for c in unicodedata.normalize('NFKC', text).lower() if c.isalnum())


def loopback_url(url):
    p = urlsplit(url)
    if p.scheme != 'http' or p.hostname not in ('127.0.0.1', '::1') or p.username or p.password or p.query or p.fragment:
        raise ValueError('vl_endpoint_must_be_literal_loopback')
    if p.path not in ('', '/', '/v1') or not p.port:
        raise ValueError('vl_endpoint_invalid')
    return url


def anchor_lines(raw):
    data = raw['res']
    result = []
    for n, (text, score, poly) in enumerate(zip(data['rec_texts'], data['rec_scores'], data['dt_polys'], strict=True)):
        if not text.strip():
            continue
        result.append(dict(lineRef=str(n), readingOrder=n, text=text, confidence=score,
                           bbox=[min(p[0] for p in poly), min(p[1] for p in poly),
                                 max(p[0] for p in poly), max(p[1] for p in poly)]))
    return result


def visual_lines(lines):
    rows=[]
    for line in sorted(lines,key=lambda l:l['bbox'][1]):
        b=line['bbox']
        if rows:
            a=rows[-1][0]['bbox']
            same=min(a[3],b[3])-max(a[1],b[1]) >= min(a[3]-a[1],b[3]-b[1])*.5
        else:same=False
        if same:rows[-1].append(line)
        else:rows.append([line])
    return [line for row in rows for line in sorted(row,key=lambda l:l['bbox'][0])]


def overlap_lines(block, lines):
    b = block['bbox']
    found = []
    for line in lines:
        a = line['bbox']
        area = (a[2]-a[0])*(a[3]-a[1])
        overlap = max(0, min(a[2], b[2])-max(a[0], b[0]))*max(0, min(a[3], b[3])-max(a[1], b[1]))
        if area > 0 and line['confidence'] >= .70 and overlap/area >= .60:
            found.append(line)
    return visual_lines(found)


class LocalRuntime:
    def __init__(self):
        for key, value in dict(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1', HF_HUB_DISABLE_TELEMETRY='1',
                               PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK='True', DO_NOT_TRACK='1').items():
            os.environ[key] = value
        for key in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy']:
            os.environ.pop(key, None)
        os.environ['NO_PROXY'] = os.environ['no_proxy'] = '127.0.0.1,localhost,::1'
        endpoint = loopback_url(os.environ['LINGGAN_PADDLE_VL_ENDPOINT'])
        root = Path(os.environ['LINGGAN_OCR_MODEL_ROOT']).resolve(strict=True)
        model = root/'PaddleOCR-VL-1.6-4bit'
        layout = root/'paddlex/official_models/PP-DocLayoutV3'
        det = root/'paddlex/official_models/PP-OCRv4_mobile_det'
        rec = root/'paddlex/official_models/PP-OCRv4_mobile_rec'
        if not all(p.is_dir() for p in [model,layout,det,rec]):
            raise ValueError('explicit_local_models_missing')
        import paddleocr
        import paddlex
        if paddleocr.__version__ != '3.7.0' or paddlex.__version__ != '3.7.2':
            raise ValueError('unsupported_receipt_adapter_version')
        from paddleocr import PaddleOCR, PaddleOCRVL
        common = dict(device='cpu', cpu_threads=4, enable_mkldnn=False)
        self.lines = PaddleOCR(text_detection_model_name='PP-OCRv4_mobile_det', text_recognition_model_name='PP-OCRv4_mobile_rec',
                              text_detection_model_dir=str(det), text_recognition_model_dir=str(rec),
                              use_doc_orientation_classify=False, use_doc_unwarping=False, use_textline_orientation=False, **common)
        self.vl = PaddleOCRVL(pipeline_version='v1.6', layout_detection_model_dir=str(layout),
                             vl_rec_model_name='PaddleOCR-VL-1.6-0.9B', vl_rec_backend='mlx-vlm-server',
                             vl_rec_server_url=endpoint, vl_rec_api_model_name=str(model), vl_rec_api_key='local-only',
                             vl_rec_max_concurrency=1, use_layout_detection=True, use_doc_orientation_classify=False,
                             use_doc_unwarping=False, use_ocr_for_image_block=True, use_queues=False, **common)
        self.capture = ReceiptCapture()
        self.capture.install(self.vl)

    def raw_lines(self, source):
        if hasattr(source, 'convert'):
            import numpy as np
            source=np.ascontiguousarray(np.asarray(source.convert('RGB'))[:,:,::-1])
        raw = list(self.lines.predict(source))[0].json
        return json.loads(raw) if isinstance(raw, str) else raw

    def raw_vl(self, source):
        if hasattr(source, 'convert'):
            import numpy as np
            source=np.ascontiguousarray(np.asarray(source.convert('RGB'))[:,:,::-1])
        self.capture.reset()
        raw = list(self.vl.predict(source, vlm_kwargs={'max_new_tokens':4096}))[0].json
        raw = json.loads(raw) if isinstance(raw,str) else raw
        return raw, self.capture.for_blocks(raw['res']['parsing_res_list']), list(self.capture.bound)

    def crop_line(self, image, block, image_sha):
        from PIL import Image
        w,h = image.size
        b = block['bbox']
        box = [max(0,math.floor(b[0])-4), max(0,math.floor(b[1])-4), min(w,math.ceil(b[2])+4), min(h,math.ceil(b[3])+4)]
        crop = image.crop(box).resize((2*(box[2]-box[0]), 2*(box[3]-box[1])), Image.Resampling.LANCZOS)
        buf = io.BytesIO(); crop.save(buf,format='PNG')
        raw = self.raw_lines(crop.convert('RGB'))
        text = '\n'.join(line['text'] for line in visual_lines(anchor_lines(raw)))
        check = dict(blockRef=block['blockRef'], imageBlobSha256=image_sha, cropBbox=box,
                     cropSha256=sha(buf.getvalue()), rawSha256=sha(encoded(raw).encode()),
                     pipelineVersion='PP-OCRv4-mobile;crop-envelope-2x-v1', kind='line_crop', text=text, finishReason=None)
        return check, dict(check=check, width=crop.width,height=crop.height,raw=raw,rawJson=encoded(raw))

    def crop_vl(self, image, block, image_sha):
        import numpy as np
        from PIL import Image
        w,h=image.size;b=block['bbox']
        box=[max(0,math.floor(b[0])-4),max(0,math.floor(b[1])-4),min(w,math.ceil(b[2])+4),min(h,math.ceil(b[3])+4)]
        crop=image.crop(box).resize((2*(box[2]-box[0]),2*(box[3]-box[1])),Image.Resampling.LANCZOS)
        buf=io.BytesIO();crop.save(buf,format='PNG')
        model=self.vl.paddlex_pipeline._pipeline.vl_rec_model
        start=len(self.capture.responses)
        response=list(model.predict([dict(image=np.ascontiguousarray(np.asarray(crop)[:,:,::-1]),query='OCR:')],max_new_tokens=4096,skip_special_tokens=True))[0]
        records=self.capture.responses[start:]
        if len(records)!=1 or records[0]['rawOutputSha256']!=sha((response.get('result')or'').encode()):
            raise ValueError('crop_receipt_mismatch')
        record=records[0];text=record['rawText'];raw=encoded(record)
        check=dict(blockRef=block['blockRef'],imageBlobSha256=image_sha,cropBbox=box,cropSha256=sha(buf.getvalue()),rawSha256=sha(raw.encode()),
                   pipelineVersion='PaddleOCR-VL-1.6;crop-envelope-2x-v1',kind='vl_crop',text=text,finishReason=record['finishReason'])
        return check,dict(check=check,width=crop.width,height=crop.height,raw=record,rawJson=raw)

    def predict(self, path, identity, role, ordinal, recheck_limit=6):
        from PIL import Image
        raw_bytes = Path(path).read_bytes()
        if sha(raw_bytes) != identity['blobSha256']:
            raise ValueError('image_identity_mismatch')
        image = Image.open(io.BytesIO(raw_bytes)).convert('RGB')
        if list(image.size) != [identity['width'],identity['height']]:
            raise ValueError('image_dimensions_mismatch')
        anchors = self.raw_lines(image)
        raw, terminations, generations = self.raw_vl(image)
        blocks = [dict(blockRef=str(b['block_id']),readingOrder=n,label=b['block_label'],text=b['block_content'],bbox=b['block_bbox'])
                  for n,b in enumerate(raw['res']['parsing_res_list'])]
        lines = anchor_lines(anchors)
        candidates = [b for b in blocks if b['label'] in ['text','doc_title','paragraph_title']
                      and 2 <= len(normalized(b['text'])) <= 3000
                      and normalized(b['text']) != normalized('\n'.join(l['text'] for l in overlap_lines(b,lines)))]
        candidates=[b for b in candidates if b['label']=='text' or (b['bbox'][3]-b['bbox'][1])/identity['height']>=.04]
        candidates.sort(key=lambda b:(b['label']=='text',-len(normalized(b['text']))))
        checks=[]; artifacts=[]
        for b in candidates[:recheck_limit]:
            check,artifact=self.crop_line(image,b,identity['blobSha256']);checks.append(check);artifacts.append(artifact)
            if normalized(check['text'])!=normalized(b['text']):
                check,artifact=self.crop_vl(image,b,identity['blobSha256']);checks.append(check);artifacts.append(artifact)
        a_json=encoded(anchors);v_json=encoded(dict(paddle=raw,generations=generations,rechecks=artifacts))
        observation=dict(schemaVersion=1,image=identity,imageRole=role,sequenceOrdinal=ordinal,
                         anchors=dict(image=identity,pipeline=dict(version='PP-OCRv4-mobile;raw-lines-v1',rawSha256=sha(a_json.encode())),lines=lines),
                         layout=dict(image=identity,pipeline=dict(version='PaddleOCR-VL-1.6;receipt-v1;mlx-4bit',rawSha256=sha(v_json.encode())),completed=True,
                                     requireTermination=True,blocks=blocks,terminations=terminations),rechecks=checks)
        return dict(schemaVersion=2,ok=True,observation=observation,anchorRawJson=a_json,vlRawJson=v_json)


def main():
    if len(sys.argv)!=3:
        raise ValueError('usage')
    meta=json.loads(Path(sys.argv[2]).read_text())
    from PIL import Image
    with Image.open(sys.argv[1]) as image:
        width,height=image.size
    meta['image'].setdefault('width',width);meta['image'].setdefault('height',height)
    result=LocalRuntime().predict(Path(sys.argv[1]),meta['image'],meta['imageRole'],meta.get('sequenceOrdinal'))
    print(encoded(result),flush=True)

if __name__=='__main__':
    try:
        main()
    except Exception:
        # No request/response/input/path is copied into ordinary worker errors.
        print('{"schemaVersion":2,"ok":false,"error":"local_vl_bridge_failed"}',flush=True)
        raise SystemExit(2)
