(() => {
 const request=window.requestAnimationFrame.bind(window),cancel=window.cancelAnimationFrame.bind(window);
 window.__frames=new Set();window.__frameCalls=0;window.__authRequests=0;
 window.requestAnimationFrame=callback=>{let id;id=request(time=>{window.__frames.delete(id);window.__frameCalls++;callback(time);});window.__frames.add(id);return id;};
 window.cancelAnimationFrame=id=>{window.__frames.delete(id);cancel(id);};
 window.fetch=()=>{window.__authRequests++;return Promise.reject(new Error('Unexpected network request in preview'));};
 const send=XMLHttpRequest.prototype.send;XMLHttpRequest.prototype.send=function(...args){window.__authRequests++;return send.apply(this,args);};
 Object.defineProperty(document,'hidden',{configurable:true,get:()=>!!window.__testHidden});
})();
