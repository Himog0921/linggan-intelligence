function makeHomeScene(container,onSelect){
  const THREE=window.THREE,labels=container.querySelector('.scene-labels');
  let renderer,frame=0,disposed=false,selected=null,playing=false;
  try{renderer=new THREE.WebGLRenderer({antialias:true,alpha:false,powerPreference:'low-power'});}catch(error){return null;}
  const styles=getComputedStyle(document.documentElement),color=name=>styles.getPropertyValue(name).trim();
  renderer.setPixelRatio(Math.min(window.devicePixelRatio||1,1.5));
  renderer.setClearColor(color('--canvas-low'));
  renderer.domElement.setAttribute('aria-label','领域地形；下方区域按钮提供等价键盘操作');
  container.prepend(renderer.domElement);
  const scene=new THREE.Scene(),camera=new THREE.OrthographicCamera(-8,8,5,-5,.1,100);
  camera.position.set(11,14,16);camera.lookAt(0,0,0);
  scene.add(new THREE.HemisphereLight(0xffffff,0x9ca8ad,2.8));
  const light=new THREE.DirectionalLight(0xffffff,2);light.position.set(-6,12,4);scene.add(light);
  const clickables=[],plates=[],htmlLabels=[];
  const outlineMaterial=new THREE.LineBasicMaterial({color:color('--status-info'),transparent:true,opacity:.2});
  DEMO.regions.forEach((region,index)=>{
    const shape=new THREE.Shape();
    const points=[[-1.6,-1.15],[.8,-1.35],[1.6,-.65],[1.48,.85],[.25,1.25],[-1.45,.72]];
    points.forEach(([x,y],i)=>i?shape.lineTo(x,y):shape.moveTo(x,y));shape.closePath();
    const geometry=new THREE.ExtrudeGeometry(shape,{depth:.22,bevelEnabled:true,bevelSegments:1,steps:1,bevelSize:.04,bevelThickness:.04});
    geometry.rotateX(-Math.PI/2);
    const material=new THREE.MeshStandardMaterial({color:region.kind==='unknown'?color('--canvas-sunken'):color('--canvas'),roughness:1,metalness:0});
    const plate=new THREE.Mesh(geometry,material);plate.position.set(region.x,.1,region.z);plate.userData.region=region.id;scene.add(plate);clickables.push(plate);plates.push(plate);
    const edges=new THREE.LineSegments(new THREE.EdgesGeometry(geometry),outlineMaterial);edges.position.copy(plate.position);scene.add(edges);
    for(let j=0;j<3;j++){
      const tile=new THREE.Mesh(new THREE.BoxGeometry(.7,.09+.07*j,.55),new THREE.MeshStandardMaterial({color:j===2?color('--canvas-sunken'):color('--canvas-low'),roughness:1}));
      tile.position.set(region.x-.7+j*.68,.4+j*.035,region.z-.25+(j%2)*.3);tile.userData.region=region.id;scene.add(tile);clickables.push(tile);
    }
    if(state.home==='normal'&&DEMO.findings.some(f=>f.region===region.id)){
      const stem=new THREE.Mesh(new THREE.CylinderGeometry(.025,.025,.72,6),new THREE.MeshBasicMaterial({color:color('--signal-mark')}));stem.position.set(region.x+.85,.7,region.z-.4);scene.add(stem);
      const pin=new THREE.Mesh(new THREE.SphereGeometry(.12,10,6),new THREE.MeshBasicMaterial({color:color('--signal-mark')}));pin.position.set(region.x+.85,1.08,region.z-.4);pin.userData.region=region.id;scene.add(pin);clickables.push(pin);
    }
    const label=document.createElement('button');label.type='button';label.className='region-label';label.textContent=region.name+(region.kind==='unknown'?' · 观察不足':'');label.dataset.region=region.id;label.addEventListener('click',()=>onSelect(region.id));labels.append(label);htmlLabels.push({label,region,position:new THREE.Vector3(region.x,.4,region.z+1.12)});
  });
  const journey=DEMO.regions.slice(0,4).map(r=>new THREE.Vector3(r.x,.7,r.z));
  const curve=new THREE.CatmullRomCurve3(journey),line=new THREE.Line(new THREE.BufferGeometry().setFromPoints(curve.getPoints(72)),new THREE.LineBasicMaterial({color:color('--status-info'),transparent:true,opacity:.42}));
  line.visible=false;scene.add(line);
  const cursor=new THREE.Mesh(new THREE.SphereGeometry(.1,8,6),new THREE.MeshBasicMaterial({color:color('--signal-mark')}));cursor.visible=false;scene.add(cursor);
  const raycaster=new THREE.Raycaster(),pointer=new THREE.Vector2();
  function draw(){if(disposed)return;renderer.render(scene,camera);for(const {label,position} of htmlLabels){const p=position.clone().project(camera);label.style.left=`${(p.x*.5+.5)*container.clientWidth}px`;label.style.top=`${(-p.y*.5+.5)*container.clientHeight}px`;}
    container.dataset.triangles=String(renderer.info.render.triangles);container.dataset.drawCalls=String(renderer.info.render.calls);container.dataset.renderer='three-webgl';container.dataset.frames=String((Number(container.dataset.frames)||0)+1);
  }
  function resize(){const w=container.clientWidth,h=container.clientHeight;if(!w||!h)return;renderer.setSize(w,h,false);const aspect=w/h,span=Math.max(9.2,14.8/aspect);camera.left=-span*aspect/2;camera.right=span*aspect/2;camera.top=span/2;camera.bottom=-span/2;camera.updateProjectionMatrix();draw();}
  function pick(event){const rect=renderer.domElement.getBoundingClientRect();pointer.set((event.clientX-rect.left)/rect.width*2-1,-(event.clientY-rect.top)/rect.height*2+1);raycaster.setFromCamera(pointer,camera);const hit=raycaster.intersectObjects(clickables)[0];if(hit)onSelect(hit.object.userData.region);}
  renderer.domElement.addEventListener('click',pick);
  const observer=new ResizeObserver(resize);observer.observe(container);resize();
  function stop(){cancelAnimationFrame(frame);playing=false;cursor.visible=false;line.visible=false;draw();}
  const visibility=()=>{if(document.hidden)stop();};document.addEventListener('visibilitychange',visibility);
  function contextLoss(event){event.preventDefault();stop();container.dataset.renderer='context-lost';container.dispatchEvent(new CustomEvent('sceneunavailable'));}
  renderer.domElement.addEventListener('webglcontextlost',contextLoss);
  return {
    select(id){selected=id;plates.forEach(p=>p.material.color.set(p.userData.region===id?color('--canvas-sunken'):(DEMO.regions.find(r=>r.id===p.userData.region).kind==='unknown'?color('--canvas-sunken'):color('--canvas'))));htmlLabels.forEach(x=>x.label.classList.toggle('active',x.region.id===id));draw();},
    replay(){stop();if(matchMedia('(prefers-reduced-motion: reduce)').matches){line.visible=true;draw();return;}playing=true;line.visible=true;cursor.visible=true;const started=performance.now();function tick(now){if(disposed||!playing)return;const t=Math.min(1,(now-started)/1600);cursor.position.copy(curve.getPoint(t));draw();if(t<1)frame=requestAnimationFrame(tick);else stop();}frame=requestAnimationFrame(tick);},
    dispose(){stop();disposed=true;observer.disconnect();document.removeEventListener('visibilitychange',visibility);renderer.domElement.removeEventListener('click',pick);renderer.domElement.removeEventListener('webglcontextlost',contextLoss);const geometries=new Set(),materials=new Set();scene.traverse(o=>{if(o.geometry)geometries.add(o.geometry);if(o.material)(Array.isArray(o.material)?o.material:[o.material]).forEach(m=>materials.add(m));});geometries.forEach(g=>g.dispose());materials.forEach(m=>m.dispose());renderer.dispose();renderer.domElement.remove();labels.replaceChildren();}
  };
}
