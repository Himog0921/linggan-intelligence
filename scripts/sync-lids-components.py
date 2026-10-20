#!/usr/bin/env python3
"""Generate/check review mirrors of the accepted four reuse classes and seven gates."""
import argparse,hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
SOURCE=ROOT/'docs/design/lids/component-reuse.json'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--review-root',type=Path,required=True);m=p.add_mutually_exclusive_group(required=True);m.add_argument('--write',action='store_true');m.add_argument('--check',action='store_true');a=p.parse_args()
 raw=SOURCE.read_bytes();s=json.loads(raw)
 if s.get('status')!='accepted' or set(s.get('routes',{}))!={'shared','adapter','business','gap'} or len(s.get('gates',[]))!=7 or len(s.get('components',[]))!=48 or s.get('formalCMPsGranted')!=0 or any(c.get('certified') is not False for c in s['components']):p.error('source must retain four classes, 48 bindings, seven gates and no automatic CMP')
 path=a.review_root/'spec/registry.json';r=json.loads(path.read_text());mirror={**s,'generatedFrom':{'path':'docs/design/lids/component-reuse.json','checkout':str(ROOT),'sha256':hashlib.sha256(raw).hexdigest(),'editPolicy':'Generated mirror; edit project source and rerun sync-lids-components.py.'}}
 cp=a.review_root/'reference-study/src/componentReview/catalog.json';c=json.loads(cp.read_text());by={v['id']:v for v in s['components']};expected={**c,'status':'accepted_four_classes_not_component_certification','normativeSource':'docs/design/lids/component-reuse.json','routes':s['routes'],'gates':s['gates'],'components':[{**v,**by[v['id']]} for v in c['components']],'generatedFrom':mirror['generatedFrom']}
 if a.check:
  if r.get('visual',{}).get('componentApplication')!=mirror or c!=expected:print('Component source/mirror drift');return 1
 else:
  r['visual']['componentApplication']=mirror;path.write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n');cp.write_text(json.dumps(expected,ensure_ascii=False,indent=2)+'\n')
 print(json.dumps({'result':'passed','standard':s['id'],'runtimeChanged':False}));return 0
if __name__=='__main__':raise SystemExit(main())
