import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const base=readFileSync(new URL('../ui/js/base.js',import.meta.url),'utf8');
const cacheSource=base.slice(base.indexOf('let instCache ='),base.indexOf('let versionCache ='));
function cache(){
 const calls=[];let renders=0,now=10000;
 const invoke=()=>new Promise((resolve,reject)=>calls.push({resolve,reject}));
 const context=vm.createContext({invoke,call:invoke,state:{route:'library'},render:()=>{renders++;},Date:{now:()=>now}});
 vm.runInContext(cacheSource,context);
 return {context,calls,run:s=>vm.runInContext(s,context),advance:()=>{now+=3000;},get renders(){return renders;}};
}
test('concurrent cold library reads share one request',async()=>{
 const c=cache();const a=c.run('listInstances()'),b=c.run('listInstances()');
 assert.equal(c.calls.length,1);c.calls[0].resolve([{id:'a'}]);assert.deepEqual(await a,await b);
});
test('repeated navigation uses fresh cache without disk requests',async()=>{
 const c=cache();const a=c.run('listInstances()');c.calls[0].resolve([{id:'a'}]);await a;
 for(let n=0;n<20;n++)await c.run('listInstances()');assert.equal(c.calls.length,1);
 c.advance();await c.run('listInstances()');await c.run('listInstances()');assert.equal(c.calls.length,2);
 c.calls[1].resolve([{id:'b'}]);await new Promise(r=>setImmediate(r));assert.equal(c.renders,1);
});
test('late read cannot overwrite a refreshed library',async()=>{
 const c=cache();const old=c.run('listInstances()');c.run('refresh()');const fresh=c.run('listInstances()');
 assert.equal(c.calls.length,2);c.calls[1].resolve([{id:'new'}]);await fresh;c.calls[0].resolve([{id:'old'}]);await old;
 assert.equal((await c.run('listInstances()'))[0].id,'new');
});
test('failed cold read can be retried',async()=>{
 const c=cache();const a=c.run('listInstances()');c.calls[0].reject(Error('disk'));await assert.rejects(a);
 const b=c.run('listInstances()');assert.equal(c.calls.length,2);c.calls[1].resolve([]);await b;
});
const boot=readFileSync(new URL('../ui/js/boot.js',import.meta.url),'utf8');
const logSource=boot.slice(boot.indexOf('const LOG_MAX_LINES'),boot.indexOf('const SKIP_REASONS'));
test('hidden-window log backlog stays bounded and retains latest lines',()=>{
 const handlers={};let frames=0;
 const context=vm.createContext({state:{logId:'game'},listen:(name,fn)=>{handlers[name]=fn;},requestAnimationFrame:()=>{frames++;}});
 vm.runInContext(logSource,context);
 for(let n=0;n<100;n++)handlers['game-log']({payload:['game',Array.from({length:1000},(_,i)=>`${n}:${i}`)]});
 assert.equal(vm.runInContext('logQueue.length',context),3000);
 assert.equal(vm.runInContext('logQueue[0]',context),'97:0');
 assert.equal(vm.runInContext('logQueue.at(-1)',context),'99:999');assert.equal(frames,1);
});

test('slow reads are fresh when they complete',async()=>{
 const c=cache();const pending=c.run('listInstances()');c.advance();
 c.calls[0].resolve([{id:'a'}]);await pending;await c.run('listInstances()');
 assert.equal(c.calls.length,1);
});
