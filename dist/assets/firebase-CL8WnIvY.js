const Dt=()=>{};var we={};/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Pe=function(e){const t=[];let n=0;for(let r=0;r<e.length;r++){let i=e.charCodeAt(r);i<128?t[n++]=i:i<2048?(t[n++]=i>>6|192,t[n++]=i&63|128):(i&64512)===55296&&r+1<e.length&&(e.charCodeAt(r+1)&64512)===56320?(i=65536+((i&1023)<<10)+(e.charCodeAt(++r)&1023),t[n++]=i>>18|240,t[n++]=i>>12&63|128,t[n++]=i>>6&63|128,t[n++]=i&63|128):(t[n++]=i>>12|224,t[n++]=i>>6&63|128,t[n++]=i&63|128)}return t},Ct=function(e){const t=[];let n=0,r=0;for(;n<e.length;){const i=e[n++];if(i<128)t[r++]=String.fromCharCode(i);else if(i>191&&i<224){const o=e[n++];t[r++]=String.fromCharCode((i&31)<<6|o&63)}else if(i>239&&i<365){const o=e[n++],a=e[n++],u=e[n++],c=((i&7)<<18|(o&63)<<12|(a&63)<<6|u&63)-65536;t[r++]=String.fromCharCode(55296+(c>>10)),t[r++]=String.fromCharCode(56320+(c&1023))}else{const o=e[n++],a=e[n++];t[r++]=String.fromCharCode((i&15)<<12|(o&63)<<6|a&63)}}return t.join("")},Le={byteToCharMap_:null,charToByteMap_:null,byteToCharMapWebSafe_:null,charToByteMapWebSafe_:null,ENCODED_VALS_BASE:"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",get ENCODED_VALS(){return this.ENCODED_VALS_BASE+"+/="},get ENCODED_VALS_WEBSAFE(){return this.ENCODED_VALS_BASE+"-_."},HAS_NATIVE_SUPPORT:typeof atob=="function",encodeByteArray(e,t){if(!Array.isArray(e))throw Error("encodeByteArray takes an array as a parameter");this.init_();const n=t?this.byteToCharMapWebSafe_:this.byteToCharMap_,r=[];for(let i=0;i<e.length;i+=3){const o=e[i],a=i+1<e.length,u=a?e[i+1]:0,c=i+2<e.length,s=c?e[i+2]:0,b=o>>2,E=(o&3)<<4|u>>4;let C=(u&15)<<2|s>>6,$=s&63;c||($=64,a||(C=64)),r.push(n[b],n[E],n[C],n[$])}return r.join("")},encodeString(e,t){return this.HAS_NATIVE_SUPPORT&&!t?btoa(e):this.encodeByteArray(Pe(e),t)},decodeString(e,t){return this.HAS_NATIVE_SUPPORT&&!t?atob(e):Ct(this.decodeStringToByteArray(e,t))},decodeStringToByteArray(e,t){this.init_();const n=t?this.charToByteMapWebSafe_:this.charToByteMap_,r=[];for(let i=0;i<e.length;){const o=n[e.charAt(i++)],u=i<e.length?n[e.charAt(i)]:0;++i;const s=i<e.length?n[e.charAt(i)]:64;++i;const E=i<e.length?n[e.charAt(i)]:64;if(++i,o==null||u==null||s==null||E==null)throw new kt;const C=o<<2|u>>4;if(r.push(C),s!==64){const $=u<<4&240|s>>2;if(r.push($),E!==64){const At=s<<6&192|E;r.push(At)}}}return r},init_(){if(!this.byteToCharMap_){this.byteToCharMap_={},this.charToByteMap_={},this.byteToCharMapWebSafe_={},this.charToByteMapWebSafe_={};for(let e=0;e<this.ENCODED_VALS.length;e++)this.byteToCharMap_[e]=this.ENCODED_VALS.charAt(e),this.charToByteMap_[this.byteToCharMap_[e]]=e,this.byteToCharMapWebSafe_[e]=this.ENCODED_VALS_WEBSAFE.charAt(e),this.charToByteMapWebSafe_[this.byteToCharMapWebSafe_[e]]=e,e>=this.ENCODED_VALS_BASE.length&&(this.charToByteMap_[this.ENCODED_VALS_WEBSAFE.charAt(e)]=e,this.charToByteMapWebSafe_[this.ENCODED_VALS.charAt(e)]=e)}}};class kt extends Error{constructor(){super(...arguments),this.name="DecodeBase64StringError"}}const Ot=function(e){const t=Pe(e);return Le.encodeByteArray(t,!0)},Fe=function(e){return Ot(e).replace(/\./g,"")},xe=function(e){try{return Le.decodeString(e,!0)}catch(t){console.error("base64Decode failed: ",t)}return null};/**
 * @license
 * Copyright 2022 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function Nt(){if(typeof self<"u")return self;if(typeof window<"u")return window;if(typeof global<"u")return global;throw new Error("Unable to locate global object.")}/**
 * @license
 * Copyright 2022 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Rt=()=>Nt().__FIREBASE_DEFAULTS__,Mt=()=>{if(typeof process>"u"||typeof we>"u")return;const e=we.__FIREBASE_DEFAULTS__;if(e)return JSON.parse(e)},Bt=()=>{if(typeof document>"u")return;let e;try{e=document.cookie.match(/__FIREBASE_DEFAULTS__=([^;]+)/)}catch{return}const t=e&&xe(e[1]);return t&&JSON.parse(t)},$t=()=>{try{return Dt()||Rt()||Mt()||Bt()}catch(e){console.info(`Unable to get __FIREBASE_DEFAULTS__ due to: ${e}`);return}},oe=()=>{var e;return(e=$t())==null?void 0:e.config};/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */class Pt{constructor(){this.reject=()=>{},this.resolve=()=>{},this.promise=new Promise((t,n)=>{this.resolve=t,this.reject=n})}wrapCallback(t){return(n,r)=>{n?this.reject(n):this.resolve(r),typeof t=="function"&&(this.promise.catch(()=>{}),t.length===1?t(n):t(n,r))}}}function Lt(){return typeof window<"u"||He()}function He(){return typeof WorkerGlobalScope<"u"&&typeof self<"u"&&self instanceof WorkerGlobalScope}function Ve(){try{return typeof indexedDB=="object"}catch{return!1}}function je(){return new Promise((e,t)=>{try{let n=!0;const r="validate-browser-context-for-indexeddb-analytics-module",i=self.indexedDB.open(r);i.onsuccess=()=>{i.result.close(),n||self.indexedDB.deleteDatabase(r),e(!0)},i.onupgradeneeded=()=>{n=!1},i.onerror=()=>{var o;t(((o=i.error)==null?void 0:o.message)||"")}}catch(n){t(n)}})}function Ft(){return!(typeof navigator>"u"||!navigator.cookieEnabled)}/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const xt="FirebaseError";class D extends Error{constructor(t,n,r){super(n),this.code=t,this.customData=r,this.name=xt,Object.setPrototypeOf(this,D.prototype),Error.captureStackTrace&&Error.captureStackTrace(this,F.prototype.create)}}class F{constructor(t,n,r){this.service=t,this.serviceName=n,this.errors=r}create(t,...n){const r=n[0]||{},i=`${this.service}/${t}`,o=this.errors[t],a=o?Ht(o,r):"Error",u=`${this.serviceName}: ${a} (${i}).`;return new D(i,u,r)}}function Ht(e,t){return e.replace(Vt,(n,r)=>{const i=t[r];return i!=null?String(i):`<${r}?>`})}const Vt=/\{\$([^}]+)}/g;function Z(e,t){if(e===t)return!0;const n=Object.keys(e),r=Object.keys(t);for(const i of n){if(!r.includes(i))return!1;const o=e[i],a=t[i];if(ye(o)&&ye(a)){if(!Z(o,a))return!1}else if(o!==a)return!1}for(const i of r)if(!n.includes(i))return!1;return!0}function ye(e){return e!==null&&typeof e=="object"}/**
 * @license
 * Copyright 2021 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function x(e){return e&&e._delegate?e._delegate:e}class w{constructor(t,n,r){this.name=t,this.instanceFactory=n,this.type=r,this.multipleInstances=!1,this.serviceProps={},this.instantiationMode="LAZY",this.onInstanceCreated=null}setInstantiationMode(t){return this.instantiationMode=t,this}setMultipleInstances(t){return this.multipleInstances=t,this}setServiceProps(t){return this.serviceProps=t,this}setInstanceCreatedCallback(t){return this.onInstanceCreated=t,this}}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const S="[DEFAULT]";/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */class jt{constructor(t,n){this.name=t,this.container=n,this.component=null,this.instances=new Map,this.instancesDeferred=new Map,this.instancesOptions=new Map,this.onInitCallbacks=new Map}get(t){const n=this.normalizeInstanceIdentifier(t);if(!this.instancesDeferred.has(n)){const r=new Pt;if(this.instancesDeferred.set(n,r),this.isInitialized(n)||this.shouldAutoInitialize())try{const i=this.getOrInitializeService({instanceIdentifier:n});i&&r.resolve(i)}catch{}}return this.instancesDeferred.get(n).promise}getImmediate(t){const n=this.normalizeInstanceIdentifier(t==null?void 0:t.identifier),r=(t==null?void 0:t.optional)??!1;if(this.isInitialized(n)||this.shouldAutoInitialize())try{return this.getOrInitializeService({instanceIdentifier:n})}catch(i){if(r)return null;throw i}else{if(r)return null;throw Error(`Service ${this.name} is not available`)}}getComponent(){return this.component}setComponent(t){if(t.name!==this.name)throw Error(`Mismatching Component ${t.name} for Provider ${this.name}.`);if(this.component)throw Error(`Component for ${this.name} has already been provided`);if(this.component=t,!!this.shouldAutoInitialize()){if(Wt(t))try{this.getOrInitializeService({instanceIdentifier:S})}catch{}for(const[n,r]of this.instancesDeferred.entries()){const i=this.normalizeInstanceIdentifier(n);try{const o=this.getOrInitializeService({instanceIdentifier:i});r.resolve(o)}catch{}}}}clearInstance(t=S){this.instancesDeferred.delete(t),this.instancesOptions.delete(t),this.instances.delete(t)}async delete(){const t=Array.from(this.instances.values());await Promise.all([...t.filter(n=>"INTERNAL"in n).map(n=>n.INTERNAL.delete()),...t.filter(n=>"_delete"in n).map(n=>n._delete())])}isComponentSet(){return this.component!=null}isInitialized(t=S){return this.instances.has(t)}getOptions(t=S){return this.instancesOptions.get(t)||{}}initialize(t={}){const{options:n={}}=t,r=this.normalizeInstanceIdentifier(t.instanceIdentifier);if(this.isInitialized(r))throw Error(`${this.name}(${r}) has already been initialized`);if(!this.isComponentSet())throw Error(`Component ${this.name} has not been registered yet`);const i=this.getOrInitializeService({instanceIdentifier:r,options:n});for(const[o,a]of this.instancesDeferred.entries()){const u=this.normalizeInstanceIdentifier(o);r===u&&a.resolve(i)}return i}onInit(t,n){const r=this.normalizeInstanceIdentifier(n),i=this.onInitCallbacks.get(r)??new Set;i.add(t),this.onInitCallbacks.set(r,i);const o=this.instances.get(r);return o&&t(o,r),()=>{i.delete(t)}}invokeOnInitCallbacks(t,n){const r=this.onInitCallbacks.get(n);if(r)for(const i of r)try{i(t,n)}catch{}}getOrInitializeService({instanceIdentifier:t,options:n={}}){let r=this.instances.get(t);if(!r&&this.component&&(r=this.component.instanceFactory(this.container,{instanceIdentifier:Ut(t),options:n}),this.instances.set(t,r),this.instancesOptions.set(t,n),this.invokeOnInitCallbacks(r,t),this.component.onInstanceCreated))try{this.component.onInstanceCreated(this.container,t,r)}catch{}return r||null}normalizeInstanceIdentifier(t=S){return this.component?this.component.multipleInstances?t:S:t}shouldAutoInitialize(){return!!this.component&&this.component.instantiationMode!=="EXPLICIT"}}function Ut(e){return e===S?void 0:e}function Wt(e){return e.instantiationMode==="EAGER"}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */class Ue{constructor(t){this.name=t,this.providers=new Map}addComponent(t){const n=this.getProvider(t.name);if(n.isComponentSet())throw new Error(`Component ${t.name} has already been registered with ${this.name}`);n.setComponent(t)}addOrOverwriteComponent(t){this.getProvider(t.name).isComponentSet()&&this.providers.delete(t.name),this.addComponent(t)}getProvider(t){if(this.providers.has(t))return this.providers.get(t);const n=new jt(t,this);return this.providers.set(t,n),n}getProviders(){return Array.from(this.providers.values())}}/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const ae=[];var d;(function(e){e[e.DEBUG=0]="DEBUG",e[e.VERBOSE=1]="VERBOSE",e[e.INFO=2]="INFO",e[e.WARN=3]="WARN",e[e.ERROR=4]="ERROR",e[e.SILENT=5]="SILENT"})(d||(d={}));const We={debug:d.DEBUG,verbose:d.VERBOSE,info:d.INFO,warn:d.WARN,error:d.ERROR,silent:d.SILENT},Kt=d.INFO,zt={[d.DEBUG]:"log",[d.VERBOSE]:"log",[d.INFO]:"info",[d.WARN]:"warn",[d.ERROR]:"error"},qt=(e,t,...n)=>{if(t<e.logLevel)return;const r=new Date().toISOString(),i=zt[t];if(i)console[i](`[${r}]  ${e.name}:`,...n);else throw new Error(`Attempted to log a message with an invalid logType (value: ${t})`)};class Gt{constructor(t){this.name=t,this._logLevel=Kt,this._logHandler=qt,this._userLogHandler=null,ae.push(this)}get logLevel(){return this._logLevel}set logLevel(t){if(!(t in d))throw new TypeError(`Invalid value "${t}" assigned to \`logLevel\``);this._logLevel=t}setLogLevel(t){this._logLevel=typeof t=="string"?We[t]:t}get logHandler(){return this._logHandler}set logHandler(t){if(typeof t!="function")throw new TypeError("Value assigned to `logHandler` must be a function");this._logHandler=t}get userLogHandler(){return this._userLogHandler}set userLogHandler(t){this._userLogHandler=t}debug(...t){this._userLogHandler&&this._userLogHandler(this,d.DEBUG,...t),this._logHandler(this,d.DEBUG,...t)}log(...t){this._userLogHandler&&this._userLogHandler(this,d.VERBOSE,...t),this._logHandler(this,d.VERBOSE,...t)}info(...t){this._userLogHandler&&this._userLogHandler(this,d.INFO,...t),this._logHandler(this,d.INFO,...t)}warn(...t){this._userLogHandler&&this._userLogHandler(this,d.WARN,...t),this._logHandler(this,d.WARN,...t)}error(...t){this._userLogHandler&&this._userLogHandler(this,d.ERROR,...t),this._logHandler(this,d.ERROR,...t)}}function Jt(e){ae.forEach(t=>{t.setLogLevel(e)})}function Yt(e,t){for(const n of ae){let r=null;t&&t.level&&(r=We[t.level]),e===null?n.userLogHandler=null:n.userLogHandler=(i,o,...a)=>{const u=a.map(c=>{if(c==null)return null;if(typeof c=="string")return c;if(typeof c=="number"||typeof c=="boolean")return c.toString();if(c instanceof Error)return c.message;try{return JSON.stringify(c)}catch{return null}}).filter(c=>c).join(" ");o>=(r??i.logLevel)&&e({level:d[o].toLowerCase(),message:u,args:a,type:i.name})}}}const Xt=(e,t)=>t.some(n=>e instanceof n);let Ie,Ee;function Qt(){return Ie||(Ie=[IDBDatabase,IDBObjectStore,IDBIndex,IDBCursor,IDBTransaction])}function Zt(){return Ee||(Ee=[IDBCursor.prototype.advance,IDBCursor.prototype.continue,IDBCursor.prototype.continuePrimaryKey])}const Ke=new WeakMap,ee=new WeakMap,ze=new WeakMap,U=new WeakMap,se=new WeakMap;function en(e){const t=new Promise((n,r)=>{const i=()=>{e.removeEventListener("success",o),e.removeEventListener("error",a)},o=()=>{n(g(e.result)),i()},a=()=>{r(e.error),i()};e.addEventListener("success",o),e.addEventListener("error",a)});return t.then(n=>{n instanceof IDBCursor&&Ke.set(n,e)}).catch(()=>{}),se.set(t,e),t}function tn(e){if(ee.has(e))return;const t=new Promise((n,r)=>{const i=()=>{e.removeEventListener("complete",o),e.removeEventListener("error",a),e.removeEventListener("abort",a)},o=()=>{n(),i()},a=()=>{r(e.error||new DOMException("AbortError","AbortError")),i()};e.addEventListener("complete",o),e.addEventListener("error",a),e.addEventListener("abort",a)});ee.set(e,t)}let te={get(e,t,n){if(e instanceof IDBTransaction){if(t==="done")return ee.get(e);if(t==="objectStoreNames")return e.objectStoreNames||ze.get(e);if(t==="store")return n.objectStoreNames[1]?void 0:n.objectStore(n.objectStoreNames[0])}return g(e[t])},set(e,t,n){return e[t]=n,!0},has(e,t){return e instanceof IDBTransaction&&(t==="done"||t==="store")?!0:t in e}};function nn(e){te=e(te)}function rn(e){return e===IDBDatabase.prototype.transaction&&!("objectStoreNames"in IDBTransaction.prototype)?function(t,...n){const r=e.call(W(this),t,...n);return ze.set(r,t.sort?t.sort():[t]),g(r)}:Zt().includes(e)?function(...t){return e.apply(W(this),t),g(Ke.get(this))}:function(...t){return g(e.apply(W(this),t))}}function on(e){return typeof e=="function"?rn(e):(e instanceof IDBTransaction&&tn(e),Xt(e,Qt())?new Proxy(e,te):e)}function g(e){if(e instanceof IDBRequest)return en(e);if(U.has(e))return U.get(e);const t=on(e);return t!==e&&(U.set(e,t),se.set(t,e)),t}const W=e=>se.get(e);function H(e,t,{blocked:n,upgrade:r,blocking:i,terminated:o}={}){const a=indexedDB.open(e,t),u=g(a);return r&&a.addEventListener("upgradeneeded",c=>{r(g(a.result),c.oldVersion,c.newVersion,g(a.transaction),c)}),n&&a.addEventListener("blocked",c=>n(c.oldVersion,c.newVersion,c)),u.then(c=>{o&&c.addEventListener("close",()=>o()),i&&c.addEventListener("versionchange",s=>i(s.oldVersion,s.newVersion,s))}).catch(()=>{}),u}function K(e,{blocked:t}={}){const n=indexedDB.deleteDatabase(e);return t&&n.addEventListener("blocked",r=>t(r.oldVersion,r)),g(n).then(()=>{})}const an=["get","getKey","getAll","getAllKeys","count"],sn=["put","add","delete","clear"],z=new Map;function Se(e,t){if(!(e instanceof IDBDatabase&&!(t in e)&&typeof t=="string"))return;if(z.get(t))return z.get(t);const n=t.replace(/FromIndex$/,""),r=t!==n,i=sn.includes(n);if(!(n in(r?IDBIndex:IDBObjectStore).prototype)||!(i||an.includes(n)))return;const o=async function(a,...u){const c=this.transaction(a,i?"readwrite":"readonly");let s=c.store;return r&&(s=s.index(u.shift())),(await Promise.all([s[n](...u),i&&c.done]))[0]};return z.set(t,o),o}nn(e=>({...e,get:(t,n,r)=>Se(t,n)||e.get(t,n,r),has:(t,n)=>!!Se(t,n)||e.has(t,n)}));/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */class cn{constructor(t){this.container=t}getPlatformInfoString(){return this.container.getProviders().map(n=>{if(un(n)){const r=n.getImmediate();return`${r.library}/${r.version}`}else return null}).filter(n=>n).join(" ")}}function un(e){const t=e.getComponent();return(t==null?void 0:t.type)==="VERSION"}const P="@firebase/app",ne="0.14.10";/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const m=new Gt("@firebase/app"),dn="@firebase/app-compat",fn="@firebase/analytics-compat",ln="@firebase/analytics",hn="@firebase/app-check-compat",pn="@firebase/app-check",gn="@firebase/auth",mn="@firebase/auth-compat",bn="@firebase/database",wn="@firebase/data-connect",yn="@firebase/database-compat",In="@firebase/functions",En="@firebase/functions-compat",Sn="@firebase/installations",_n="@firebase/installations-compat",Tn="@firebase/messaging",vn="@firebase/messaging-compat",An="@firebase/performance",Dn="@firebase/performance-compat",Cn="@firebase/remote-config",kn="@firebase/remote-config-compat",On="@firebase/storage",Nn="@firebase/storage-compat",Rn="@firebase/firestore",Mn="@firebase/ai",Bn="@firebase/firestore-compat",$n="firebase",Pn="12.11.0";/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const N="[DEFAULT]",Ln={[P]:"fire-core",[dn]:"fire-core-compat",[ln]:"fire-analytics",[fn]:"fire-analytics-compat",[pn]:"fire-app-check",[hn]:"fire-app-check-compat",[gn]:"fire-auth",[mn]:"fire-auth-compat",[bn]:"fire-rtdb",[wn]:"fire-data-connect",[yn]:"fire-rtdb-compat",[In]:"fire-fn",[En]:"fire-fn-compat",[Sn]:"fire-iid",[_n]:"fire-iid-compat",[Tn]:"fire-fcm",[vn]:"fire-fcm-compat",[An]:"fire-perf",[Dn]:"fire-perf-compat",[Cn]:"fire-rc",[kn]:"fire-rc-compat",[On]:"fire-gcs",[Nn]:"fire-gcs-compat",[Rn]:"fire-fst",[Bn]:"fire-fst-compat",[Mn]:"fire-vertex","fire-js":"fire-js",[$n]:"fire-js-all"};/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const y=new Map,k=new Map,O=new Map;function re(e,t){try{e.container.addComponent(t)}catch(n){m.debug(`Component ${t.name} failed to register with FirebaseApp ${e.name}`,n)}}function Fn(e,t){e.container.addOrOverwriteComponent(t)}function I(e){const t=e.name;if(O.has(t))return m.debug(`There were multiple attempts to register component ${t}.`),!1;O.set(t,e);for(const n of y.values())re(n,e);for(const n of k.values())re(n,e);return!0}function B(e,t){const n=e.container.getProvider("heartbeat").getImmediate({optional:!0});return n&&n.triggerHeartbeat(),e.container.getProvider(t)}function xn(e,t,n=N){B(e,t).clearInstance(n)}function ce(e){return e.options!==void 0}function qe(e){return ce(e)?!1:"authIdToken"in e||"appCheckToken"in e||"releaseOnDeref"in e||"automaticDataCollectionEnabled"in e}function Hn(e){return e==null?!1:e.settings!==void 0}function Vn(){O.clear()}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const jn={"no-app":"No Firebase App '{$appName}' has been created - call initializeApp() first","bad-app-name":"Illegal App name: '{$appName}'","duplicate-app":"Firebase App named '{$appName}' already exists with different options or config","app-deleted":"Firebase App named '{$appName}' already deleted","server-app-deleted":"Firebase Server App has been deleted","no-options":"Need to provide options, when not being deployed to hosting via source.","invalid-app-argument":"firebase.{$appName}() takes either no argument or a Firebase App instance.","invalid-log-argument":"First argument to `onLog` must be null or a function.","idb-open":"Error thrown when opening IndexedDB. Original error: {$originalErrorMessage}.","idb-get":"Error thrown when reading from IndexedDB. Original error: {$originalErrorMessage}.","idb-set":"Error thrown when writing to IndexedDB. Original error: {$originalErrorMessage}.","idb-delete":"Error thrown when deleting from IndexedDB. Original error: {$originalErrorMessage}.","finalization-registry-not-supported":"FirebaseServerApp deleteOnDeref field defined but the JS runtime does not support FinalizationRegistry.","invalid-server-app-environment":"FirebaseServerApp is not for use in browser environments."},l=new F("app","Firebase",jn);/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */class Ge{constructor(t,n,r){this._isDeleted=!1,this._options={...t},this._config={...n},this._name=n.name,this._automaticDataCollectionEnabled=n.automaticDataCollectionEnabled,this._container=r,this.container.addComponent(new w("app",()=>this,"PUBLIC"))}get automaticDataCollectionEnabled(){return this.checkDestroyed(),this._automaticDataCollectionEnabled}set automaticDataCollectionEnabled(t){this.checkDestroyed(),this._automaticDataCollectionEnabled=t}get name(){return this.checkDestroyed(),this._name}get options(){return this.checkDestroyed(),this._options}get config(){return this.checkDestroyed(),this._config}get container(){return this._container}get isDeleted(){return this._isDeleted}set isDeleted(t){this._isDeleted=t}checkDestroyed(){if(this.isDeleted)throw l.create("app-deleted",{appName:this._name})}}/**
 * @license
 * Copyright 2023 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function _e(e,t){const n=xe(e.split(".")[1]);if(n===null){console.error(`FirebaseServerApp ${t} is invalid: second part could not be parsed.`);return}if(JSON.parse(n).exp===void 0){console.error(`FirebaseServerApp ${t} is invalid: expiration claim could not be parsed`);return}const i=JSON.parse(n).exp*1e3,o=new Date().getTime();i-o<=0&&console.error(`FirebaseServerApp ${t} is invalid: the token has expired.`)}class Un extends Ge{constructor(t,n,r,i){const o=n.automaticDataCollectionEnabled!==void 0?n.automaticDataCollectionEnabled:!0,a={name:r,automaticDataCollectionEnabled:o};if(t.apiKey!==void 0)super(t,a,i);else{const u=t;super(u.options,a,i)}this._serverConfig={automaticDataCollectionEnabled:o,...n},this._serverConfig.authIdToken&&_e(this._serverConfig.authIdToken,"authIdToken"),this._serverConfig.appCheckToken&&_e(this._serverConfig.appCheckToken,"appCheckToken"),this._finalizationRegistry=null,typeof FinalizationRegistry<"u"&&(this._finalizationRegistry=new FinalizationRegistry(()=>{this.automaticCleanup()})),this._refCount=0,this.incRefCount(this._serverConfig.releaseOnDeref),this._serverConfig.releaseOnDeref=void 0,n.releaseOnDeref=void 0,h(P,ne,"serverapp")}toJSON(){}get refCount(){return this._refCount}incRefCount(t){this.isDeleted||(this._refCount++,t!==void 0&&this._finalizationRegistry!==null&&this._finalizationRegistry.register(t,this))}decRefCount(){return this.isDeleted?0:--this._refCount}automaticCleanup(){Xe(this)}get settings(){return this.checkDestroyed(),this._serverConfig}checkDestroyed(){if(this.isDeleted)throw l.create("server-app-deleted")}}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Wn=Pn;function Je(e,t={}){let n=e;typeof t!="object"&&(t={name:t});const r={name:N,automaticDataCollectionEnabled:!0,...t},i=r.name;if(typeof i!="string"||!i)throw l.create("bad-app-name",{appName:String(i)});if(n||(n=oe()),!n)throw l.create("no-options");const o=y.get(i);if(o){if(Z(n,o.options)&&Z(r,o.config))return o;throw l.create("duplicate-app",{appName:i})}const a=new Ue(i);for(const c of O.values())a.addComponent(c);const u=new Ge(n,r,a);return y.set(i,u),u}function Kn(e,t={}){if(Lt()&&!He())throw l.create("invalid-server-app-environment");let n,r=t||{};if(e&&(ce(e)?n=e.options:qe(e)?r=e:n=e),r.automaticDataCollectionEnabled===void 0&&(r.automaticDataCollectionEnabled=!0),n||(n=oe()),!n)throw l.create("no-options");const i={...r,...n};i.releaseOnDeref!==void 0&&delete i.releaseOnDeref;const o=b=>[...b].reduce((E,C)=>Math.imul(31,E)+C.charCodeAt(0)|0,0);if(r.releaseOnDeref!==void 0&&typeof FinalizationRegistry>"u")throw l.create("finalization-registry-not-supported",{});const a=""+o(JSON.stringify(i)),u=k.get(a);if(u)return u.incRefCount(r.releaseOnDeref),u;const c=new Ue(a);for(const b of O.values())c.addComponent(b);const s=new Un(n,r,a,c);return k.set(a,s),s}function Ye(e=N){const t=y.get(e);if(!t&&e===N&&oe())return Je();if(!t)throw l.create("no-app",{appName:e});return t}function zn(){return Array.from(y.values())}async function Xe(e){let t=!1;const n=e.name;y.has(n)?(t=!0,y.delete(n)):k.has(n)&&e.decRefCount()<=0&&(k.delete(n),t=!0),t&&(await Promise.all(e.container.getProviders().map(r=>r.delete())),e.isDeleted=!0)}function h(e,t,n){let r=Ln[e]??e;n&&(r+=`-${n}`);const i=r.match(/\s|\//),o=t.match(/\s|\//);if(i||o){const a=[`Unable to register library "${r}" with version "${t}":`];i&&a.push(`library name "${r}" contains illegal characters (whitespace or "/")`),i&&o&&a.push("and"),o&&a.push(`version name "${t}" contains illegal characters (whitespace or "/")`),m.warn(a.join(" "));return}I(new w(`${r}-version`,()=>({library:r,version:t}),"VERSION"))}function qn(e,t){if(e!==null&&typeof e!="function")throw l.create("invalid-log-argument");Yt(e,t)}function Gn(e){Jt(e)}/**
 * @license
 * Copyright 2021 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Jn="firebase-heartbeat-database",Yn=1,R="firebase-heartbeat-store";let q=null;function Qe(){return q||(q=H(Jn,Yn,{upgrade:(e,t)=>{switch(t){case 0:try{e.createObjectStore(R)}catch(n){console.warn(n)}}}}).catch(e=>{throw l.create("idb-open",{originalErrorMessage:e.message})})),q}async function Xn(e){try{const n=(await Qe()).transaction(R),r=await n.objectStore(R).get(Ze(e));return await n.done,r}catch(t){if(t instanceof D)m.warn(t.message);else{const n=l.create("idb-get",{originalErrorMessage:t==null?void 0:t.message});m.warn(n.message)}}}async function Te(e,t){try{const r=(await Qe()).transaction(R,"readwrite");await r.objectStore(R).put(t,Ze(e)),await r.done}catch(n){if(n instanceof D)m.warn(n.message);else{const r=l.create("idb-set",{originalErrorMessage:n==null?void 0:n.message});m.warn(r.message)}}}function Ze(e){return`${e.name}!${e.options.appId}`}/**
 * @license
 * Copyright 2021 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Qn=1024,Zn=30;class er{constructor(t){this.container=t,this._heartbeatsCache=null;const n=this.container.getProvider("app").getImmediate();this._storage=new nr(n),this._heartbeatsCachePromise=this._storage.read().then(r=>(this._heartbeatsCache=r,r))}async triggerHeartbeat(){var t,n;try{const i=this.container.getProvider("platform-logger").getImmediate().getPlatformInfoString(),o=ve();if(((t=this._heartbeatsCache)==null?void 0:t.heartbeats)==null&&(this._heartbeatsCache=await this._heartbeatsCachePromise,((n=this._heartbeatsCache)==null?void 0:n.heartbeats)==null)||this._heartbeatsCache.lastSentHeartbeatDate===o||this._heartbeatsCache.heartbeats.some(a=>a.date===o))return;if(this._heartbeatsCache.heartbeats.push({date:o,agent:i}),this._heartbeatsCache.heartbeats.length>Zn){const a=rr(this._heartbeatsCache.heartbeats);this._heartbeatsCache.heartbeats.splice(a,1)}return this._storage.overwrite(this._heartbeatsCache)}catch(r){m.warn(r)}}async getHeartbeatsHeader(){var t;try{if(this._heartbeatsCache===null&&await this._heartbeatsCachePromise,((t=this._heartbeatsCache)==null?void 0:t.heartbeats)==null||this._heartbeatsCache.heartbeats.length===0)return"";const n=ve(),{heartbeatsToSend:r,unsentEntries:i}=tr(this._heartbeatsCache.heartbeats),o=Fe(JSON.stringify({version:2,heartbeats:r}));return this._heartbeatsCache.lastSentHeartbeatDate=n,i.length>0?(this._heartbeatsCache.heartbeats=i,await this._storage.overwrite(this._heartbeatsCache)):(this._heartbeatsCache.heartbeats=[],this._storage.overwrite(this._heartbeatsCache)),o}catch(n){return m.warn(n),""}}}function ve(){return new Date().toISOString().substring(0,10)}function tr(e,t=Qn){const n=[];let r=e.slice();for(const i of e){const o=n.find(a=>a.agent===i.agent);if(o){if(o.dates.push(i.date),Ae(n)>t){o.dates.pop();break}}else if(n.push({agent:i.agent,dates:[i.date]}),Ae(n)>t){n.pop();break}r=r.slice(1)}return{heartbeatsToSend:n,unsentEntries:r}}class nr{constructor(t){this.app=t,this._canUseIndexedDBPromise=this.runIndexedDBEnvironmentCheck()}async runIndexedDBEnvironmentCheck(){return Ve()?je().then(()=>!0).catch(()=>!1):!1}async read(){if(await this._canUseIndexedDBPromise){const n=await Xn(this.app);return n!=null&&n.heartbeats?n:{heartbeats:[]}}else return{heartbeats:[]}}async overwrite(t){if(await this._canUseIndexedDBPromise){const r=await this.read();return Te(this.app,{lastSentHeartbeatDate:t.lastSentHeartbeatDate??r.lastSentHeartbeatDate,heartbeats:t.heartbeats})}else return}async add(t){if(await this._canUseIndexedDBPromise){const r=await this.read();return Te(this.app,{lastSentHeartbeatDate:t.lastSentHeartbeatDate??r.lastSentHeartbeatDate,heartbeats:[...r.heartbeats,...t.heartbeats]})}else return}}function Ae(e){return Fe(JSON.stringify({version:2,heartbeats:e})).length}function rr(e){if(e.length===0)return-1;let t=0,n=e[0].date;for(let r=1;r<e.length;r++)e[r].date<n&&(n=e[r].date,t=r);return t}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function ir(e){I(new w("platform-logger",t=>new cn(t),"PRIVATE")),I(new w("heartbeat",t=>new er(t),"PRIVATE")),h(P,ne,e),h(P,ne,"esm2020"),h("fire-js","")}ir("");var or="firebase",ar="12.11.0";/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */h(or,ar,"app");const $i=Object.freeze(Object.defineProperty({__proto__:null,FirebaseError:D,SDK_VERSION:Wn,_DEFAULT_ENTRY_NAME:N,_addComponent:re,_addOrOverwriteComponent:Fn,_apps:y,_clearComponents:Vn,_components:O,_getProvider:B,_isFirebaseApp:ce,_isFirebaseServerApp:Hn,_isFirebaseServerAppSettings:qe,_registerComponent:I,_removeServiceInstance:xn,_serverApps:k,deleteApp:Xe,getApp:Ye,getApps:zn,initializeApp:Je,initializeServerApp:Kn,onLog:qn,registerVersion:h,setLogLevel:Gn},Symbol.toStringTag,{value:"Module"})),et="@firebase/installations",ue="0.6.21";/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const tt=1e4,nt=`w:${ue}`,rt="FIS_v2",sr="https://firebaseinstallations.googleapis.com/v1",cr=3600*1e3,ur="installations",dr="Installations";/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const fr={"missing-app-config-values":'Missing App configuration value: "{$valueName}"',"not-registered":"Firebase Installation is not registered.","installation-not-found":"Firebase Installation not found.","request-failed":'{$requestName} request failed with error "{$serverCode} {$serverStatus}: {$serverMessage}"',"app-offline":"Could not process request. Application offline.","delete-pending-registration":"Can't delete installation while there is a pending registration request."},T=new F(ur,dr,fr);function it(e){return e instanceof D&&e.code.includes("request-failed")}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function ot({projectId:e}){return`${sr}/projects/${e}/installations`}function at(e){return{token:e.token,requestStatus:2,expiresIn:hr(e.expiresIn),creationTime:Date.now()}}async function st(e,t){const r=(await t.json()).error;return T.create("request-failed",{requestName:e,serverCode:r.code,serverMessage:r.message,serverStatus:r.status})}function ct({apiKey:e}){return new Headers({"Content-Type":"application/json",Accept:"application/json","x-goog-api-key":e})}function lr(e,{refreshToken:t}){const n=ct(e);return n.append("Authorization",pr(t)),n}async function ut(e){const t=await e();return t.status>=500&&t.status<600?e():t}function hr(e){return Number(e.replace("s","000"))}function pr(e){return`${rt} ${e}`}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function gr({appConfig:e,heartbeatServiceProvider:t},{fid:n}){const r=ot(e),i=ct(e),o=t.getImmediate({optional:!0});if(o){const s=await o.getHeartbeatsHeader();s&&i.append("x-firebase-client",s)}const a={fid:n,authVersion:rt,appId:e.appId,sdkVersion:nt},u={method:"POST",headers:i,body:JSON.stringify(a)},c=await ut(()=>fetch(r,u));if(c.ok){const s=await c.json();return{fid:s.fid||n,registrationStatus:2,refreshToken:s.refreshToken,authToken:at(s.authToken)}}else throw await st("Create Installation",c)}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function dt(e){return new Promise(t=>{setTimeout(t,e)})}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function mr(e){return btoa(String.fromCharCode(...e)).replace(/\+/g,"-").replace(/\//g,"_")}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const br=/^[cdef][\w-]{21}$/,ie="";function wr(){try{const e=new Uint8Array(17);(self.crypto||self.msCrypto).getRandomValues(e),e[0]=112+e[0]%16;const n=yr(e);return br.test(n)?n:ie}catch{return ie}}function yr(e){return mr(e).substr(0,22)}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function V(e){return`${e.appName}!${e.appId}`}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const ft=new Map;function lt(e,t){const n=V(e);ht(n,t),Ir(n,t)}function ht(e,t){const n=ft.get(e);if(n)for(const r of n)r(t)}function Ir(e,t){const n=Er();n&&n.postMessage({key:e,fid:t}),Sr()}let _=null;function Er(){return!_&&"BroadcastChannel"in self&&(_=new BroadcastChannel("[Firebase] FID Change"),_.onmessage=e=>{ht(e.data.key,e.data.fid)}),_}function Sr(){ft.size===0&&_&&(_.close(),_=null)}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const _r="firebase-installations-database",Tr=1,v="firebase-installations-store";let G=null;function de(){return G||(G=H(_r,Tr,{upgrade:(e,t)=>{switch(t){case 0:e.createObjectStore(v)}}})),G}async function L(e,t){const n=V(e),i=(await de()).transaction(v,"readwrite"),o=i.objectStore(v),a=await o.get(n);return await o.put(t,n),await i.done,(!a||a.fid!==t.fid)&&lt(e,t.fid),t}async function pt(e){const t=V(e),r=(await de()).transaction(v,"readwrite");await r.objectStore(v).delete(t),await r.done}async function j(e,t){const n=V(e),i=(await de()).transaction(v,"readwrite"),o=i.objectStore(v),a=await o.get(n),u=t(a);return u===void 0?await o.delete(n):await o.put(u,n),await i.done,u&&(!a||a.fid!==u.fid)&&lt(e,u.fid),u}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function fe(e){let t;const n=await j(e.appConfig,r=>{const i=vr(r),o=Ar(e,i);return t=o.registrationPromise,o.installationEntry});return n.fid===ie?{installationEntry:await t}:{installationEntry:n,registrationPromise:t}}function vr(e){const t=e||{fid:wr(),registrationStatus:0};return gt(t)}function Ar(e,t){if(t.registrationStatus===0){if(!navigator.onLine){const i=Promise.reject(T.create("app-offline"));return{installationEntry:t,registrationPromise:i}}const n={fid:t.fid,registrationStatus:1,registrationTime:Date.now()},r=Dr(e,n);return{installationEntry:n,registrationPromise:r}}else return t.registrationStatus===1?{installationEntry:t,registrationPromise:Cr(e)}:{installationEntry:t}}async function Dr(e,t){try{const n=await gr(e,t);return L(e.appConfig,n)}catch(n){throw it(n)&&n.customData.serverCode===409?await pt(e.appConfig):await L(e.appConfig,{fid:t.fid,registrationStatus:0}),n}}async function Cr(e){let t=await De(e.appConfig);for(;t.registrationStatus===1;)await dt(100),t=await De(e.appConfig);if(t.registrationStatus===0){const{installationEntry:n,registrationPromise:r}=await fe(e);return r||n}return t}function De(e){return j(e,t=>{if(!t)throw T.create("installation-not-found");return gt(t)})}function gt(e){return kr(e)?{fid:e.fid,registrationStatus:0}:e}function kr(e){return e.registrationStatus===1&&e.registrationTime+tt<Date.now()}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function Or({appConfig:e,heartbeatServiceProvider:t},n){const r=Nr(e,n),i=lr(e,n),o=t.getImmediate({optional:!0});if(o){const s=await o.getHeartbeatsHeader();s&&i.append("x-firebase-client",s)}const a={installation:{sdkVersion:nt,appId:e.appId}},u={method:"POST",headers:i,body:JSON.stringify(a)},c=await ut(()=>fetch(r,u));if(c.ok){const s=await c.json();return at(s)}else throw await st("Generate Auth Token",c)}function Nr(e,{fid:t}){return`${ot(e)}/${t}/authTokens:generate`}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function le(e,t=!1){let n;const r=await j(e.appConfig,o=>{if(!mt(o))throw T.create("not-registered");const a=o.authToken;if(!t&&Br(a))return o;if(a.requestStatus===1)return n=Rr(e,t),o;{if(!navigator.onLine)throw T.create("app-offline");const u=Pr(o);return n=Mr(e,u),u}});return n?await n:r.authToken}async function Rr(e,t){let n=await Ce(e.appConfig);for(;n.authToken.requestStatus===1;)await dt(100),n=await Ce(e.appConfig);const r=n.authToken;return r.requestStatus===0?le(e,t):r}function Ce(e){return j(e,t=>{if(!mt(t))throw T.create("not-registered");const n=t.authToken;return Lr(n)?{...t,authToken:{requestStatus:0}}:t})}async function Mr(e,t){try{const n=await Or(e,t),r={...t,authToken:n};return await L(e.appConfig,r),n}catch(n){if(it(n)&&(n.customData.serverCode===401||n.customData.serverCode===404))await pt(e.appConfig);else{const r={...t,authToken:{requestStatus:0}};await L(e.appConfig,r)}throw n}}function mt(e){return e!==void 0&&e.registrationStatus===2}function Br(e){return e.requestStatus===2&&!$r(e)}function $r(e){const t=Date.now();return t<e.creationTime||e.creationTime+e.expiresIn<t+cr}function Pr(e){const t={requestStatus:1,requestTime:Date.now()};return{...e,authToken:t}}function Lr(e){return e.requestStatus===1&&e.requestTime+tt<Date.now()}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function Fr(e){const t=e,{installationEntry:n,registrationPromise:r}=await fe(t);return r?r.catch(console.error):le(t).catch(console.error),n.fid}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function xr(e,t=!1){const n=e;return await Hr(n),(await le(n,t)).token}async function Hr(e){const{registrationPromise:t}=await fe(e);t&&await t}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function Vr(e){if(!e||!e.options)throw J("App Configuration");if(!e.name)throw J("App Name");const t=["projectId","apiKey","appId"];for(const n of t)if(!e.options[n])throw J(n);return{appName:e.name,projectId:e.options.projectId,apiKey:e.options.apiKey,appId:e.options.appId}}function J(e){return T.create("missing-app-config-values",{valueName:e})}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const bt="installations",jr="installations-internal",Ur=e=>{const t=e.getProvider("app").getImmediate(),n=Vr(t),r=B(t,"heartbeat");return{app:t,appConfig:n,heartbeatServiceProvider:r,_delete:()=>Promise.resolve()}},Wr=e=>{const t=e.getProvider("app").getImmediate(),n=B(t,bt).getImmediate();return{getId:()=>Fr(n),getToken:i=>xr(n,i)}};function Kr(){I(new w(bt,Ur,"PUBLIC")),I(new w(jr,Wr,"PRIVATE"))}Kr();h(et,ue);h(et,ue,"esm2020");/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const zr="/firebase-messaging-sw.js",qr="/firebase-cloud-messaging-push-scope",wt="BDOU99-h67HcA6JeFXHbSNMu7e2yNNu3RzoMj8TM4W88jITfq7ZmPvIM1Iv-4_l2LxQcYwhqby2xGpWwzjfAnG4",Gr="https://fcmregistrations.googleapis.com/v1",yt="google.c.a.c_id",Jr="google.c.a.c_l",Yr="google.c.a.ts",Xr="google.c.a.e",ke=1e4;var Oe;(function(e){e[e.DATA_MESSAGE=1]="DATA_MESSAGE",e[e.DISPLAY_NOTIFICATION=3]="DISPLAY_NOTIFICATION"})(Oe||(Oe={}));/**
 * @license
 * Copyright 2018 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */var M;(function(e){e.PUSH_RECEIVED="push-received",e.NOTIFICATION_CLICKED="notification-clicked"})(M||(M={}));/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function p(e){const t=new Uint8Array(e);return btoa(String.fromCharCode(...t)).replace(/=/g,"").replace(/\+/g,"-").replace(/\//g,"_")}function Qr(e){const t="=".repeat((4-e.length%4)%4),n=(e+t).replace(/\-/g,"+").replace(/_/g,"/"),r=atob(n),i=new Uint8Array(r.length);for(let o=0;o<r.length;++o)i[o]=r.charCodeAt(o);return i}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Y="fcm_token_details_db",Zr=5,Ne="fcm_token_object_Store";async function ei(e){if("databases"in indexedDB&&!(await indexedDB.databases()).map(o=>o.name).includes(Y))return null;let t=null;return(await H(Y,Zr,{upgrade:async(r,i,o,a)=>{if(i<2||!r.objectStoreNames.contains(Ne))return;const u=a.objectStore(Ne),c=await u.index("fcmSenderId").get(e);if(await u.clear(),!!c){if(i===2){const s=c;if(!s.auth||!s.p256dh||!s.endpoint)return;t={token:s.fcmToken,createTime:s.createTime??Date.now(),subscriptionOptions:{auth:s.auth,p256dh:s.p256dh,endpoint:s.endpoint,swScope:s.swScope,vapidKey:typeof s.vapidKey=="string"?s.vapidKey:p(s.vapidKey)}}}else if(i===3){const s=c;t={token:s.fcmToken,createTime:s.createTime,subscriptionOptions:{auth:p(s.auth),p256dh:p(s.p256dh),endpoint:s.endpoint,swScope:s.swScope,vapidKey:p(s.vapidKey)}}}else if(i===4){const s=c;t={token:s.fcmToken,createTime:s.createTime,subscriptionOptions:{auth:p(s.auth),p256dh:p(s.p256dh),endpoint:s.endpoint,swScope:s.swScope,vapidKey:p(s.vapidKey)}}}}}})).close(),await K(Y),await K("fcm_vapid_details_db"),await K("undefined"),ti(t)?t:null}function ti(e){if(!e||!e.subscriptionOptions)return!1;const{subscriptionOptions:t}=e;return typeof e.createTime=="number"&&e.createTime>0&&typeof e.token=="string"&&e.token.length>0&&typeof t.auth=="string"&&t.auth.length>0&&typeof t.p256dh=="string"&&t.p256dh.length>0&&typeof t.endpoint=="string"&&t.endpoint.length>0&&typeof t.swScope=="string"&&t.swScope.length>0&&typeof t.vapidKey=="string"&&t.vapidKey.length>0}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const ni="firebase-messaging-database",ri=1,A="firebase-messaging-store";let X=null;function he(){return X||(X=H(ni,ri,{upgrade:(e,t)=>{switch(t){case 0:e.createObjectStore(A)}}})),X}async function It(e){const t=ge(e),r=await(await he()).transaction(A).objectStore(A).get(t);if(r)return r;{const i=await ei(e.appConfig.senderId);if(i)return await pe(e,i),i}}async function pe(e,t){const n=ge(e),i=(await he()).transaction(A,"readwrite");return await i.objectStore(A).put(t,n),await i.done,t}async function ii(e){const t=ge(e),r=(await he()).transaction(A,"readwrite");await r.objectStore(A).delete(t),await r.done}function ge({appConfig:e}){return e.appId}/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const oi={"missing-app-config-values":'Missing App configuration value: "{$valueName}"',"only-available-in-window":"This method is available in a Window context.","only-available-in-sw":"This method is available in a service worker context.","permission-default":"The notification permission was not granted and dismissed instead.","permission-blocked":"The notification permission was not granted and blocked instead.","unsupported-browser":"This browser doesn't support the API's required to use the Firebase SDK.","indexed-db-unsupported":"This browser doesn't support indexedDb.open() (ex. Safari iFrame, Firefox Private Browsing, etc)","failed-service-worker-registration":"We are unable to register the default service worker. {$browserErrorMessage}","token-subscribe-failed":"A problem occurred while subscribing the user to FCM: {$errorInfo}","token-subscribe-no-token":"FCM returned no token when subscribing the user to push.","token-unsubscribe-failed":"A problem occurred while unsubscribing the user from FCM: {$errorInfo}","token-update-failed":"A problem occurred while updating the user from FCM: {$errorInfo}","token-update-no-token":"FCM returned no token when updating the user to push.","use-sw-after-get-token":"The useServiceWorker() method may only be called once and must be called before calling getToken() to ensure your service worker is used.","invalid-sw-registration":"The input to useServiceWorker() must be a ServiceWorkerRegistration.","invalid-bg-handler":"The input to setBackgroundMessageHandler() must be a function.","invalid-vapid-key":"The public VAPID key must be a string.","use-vapid-key-after-get-token":"The usePublicVapidKey() method may only be called once and must be called before calling getToken() to ensure your VAPID key is used."},f=new F("messaging","Messaging",oi);/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function ai(e,t){const n=await be(e),r=St(t),i={method:"POST",headers:n,body:JSON.stringify(r)};let o;try{o=await(await fetch(me(e.appConfig),i)).json()}catch(a){throw f.create("token-subscribe-failed",{errorInfo:a==null?void 0:a.toString()})}if(o.error){const a=o.error.message;throw f.create("token-subscribe-failed",{errorInfo:a})}if(!o.token)throw f.create("token-subscribe-no-token");return o.token}async function si(e,t){const n=await be(e),r=St(t.subscriptionOptions),i={method:"PATCH",headers:n,body:JSON.stringify(r)};let o;try{o=await(await fetch(`${me(e.appConfig)}/${t.token}`,i)).json()}catch(a){throw f.create("token-update-failed",{errorInfo:a==null?void 0:a.toString()})}if(o.error){const a=o.error.message;throw f.create("token-update-failed",{errorInfo:a})}if(!o.token)throw f.create("token-update-no-token");return o.token}async function Et(e,t){const r={method:"DELETE",headers:await be(e)};try{const o=await(await fetch(`${me(e.appConfig)}/${t}`,r)).json();if(o.error){const a=o.error.message;throw f.create("token-unsubscribe-failed",{errorInfo:a})}}catch(i){throw f.create("token-unsubscribe-failed",{errorInfo:i==null?void 0:i.toString()})}}function me({projectId:e}){return`${Gr}/projects/${e}/registrations`}async function be({appConfig:e,installations:t}){const n=await t.getToken();return new Headers({"Content-Type":"application/json",Accept:"application/json","x-goog-api-key":e.apiKey,"x-goog-firebase-installations-auth":`FIS ${n}`})}function St({p256dh:e,auth:t,endpoint:n,vapidKey:r}){const i={web:{endpoint:n,auth:t,p256dh:e}};return r!==wt&&(i.web.applicationPubKey=r),i}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const ci=10080*60*1e3;async function ui(e){const t=await li(e.swRegistration,e.vapidKey),n={vapidKey:e.vapidKey,swScope:e.swRegistration.scope,endpoint:t.endpoint,auth:p(t.getKey("auth")),p256dh:p(t.getKey("p256dh"))},r=await It(e.firebaseDependencies);if(r){if(hi(r.subscriptionOptions,n))return Date.now()>=r.createTime+ci?fi(e,{token:r.token,createTime:Date.now(),subscriptionOptions:n}):r.token;try{await Et(e.firebaseDependencies,r.token)}catch(i){console.warn(i)}return Re(e.firebaseDependencies,n)}else return Re(e.firebaseDependencies,n)}async function di(e){const t=await It(e.firebaseDependencies);t&&(await Et(e.firebaseDependencies,t.token),await ii(e.firebaseDependencies));const n=await e.swRegistration.pushManager.getSubscription();return n?n.unsubscribe():!0}async function fi(e,t){try{const n=await si(e.firebaseDependencies,t),r={...t,token:n,createTime:Date.now()};return await pe(e.firebaseDependencies,r),n}catch(n){throw n}}async function Re(e,t){const r={token:await ai(e,t),createTime:Date.now(),subscriptionOptions:t};return await pe(e,r),r.token}async function li(e,t){const n=await e.pushManager.getSubscription();return n||e.pushManager.subscribe({userVisibleOnly:!0,applicationServerKey:Qr(t)})}function hi(e,t){const n=t.vapidKey===e.vapidKey,r=t.endpoint===e.endpoint,i=t.auth===e.auth,o=t.p256dh===e.p256dh;return n&&r&&i&&o}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function Me(e){const t={from:e.from,collapseKey:e.collapse_key,messageId:e.fcmMessageId};return pi(t,e),gi(t,e),mi(t,e),t}function pi(e,t){if(!t.notification)return;e.notification={};const n=t.notification.title;n&&(e.notification.title=n);const r=t.notification.body;r&&(e.notification.body=r);const i=t.notification.image;i&&(e.notification.image=i);const o=t.notification.icon;o&&(e.notification.icon=o)}function gi(e,t){t.data&&(e.data=t.data)}function mi(e,t){var i,o,a,u;if(!t.fcmOptions&&!((i=t.notification)!=null&&i.click_action))return;e.fcmOptions={};const n=((o=t.fcmOptions)==null?void 0:o.link)??((a=t.notification)==null?void 0:a.click_action);n&&(e.fcmOptions.link=n);const r=(u=t.fcmOptions)==null?void 0:u.analytics_label;r&&(e.fcmOptions.analyticsLabel=r)}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function bi(e){return typeof e=="object"&&!!e&&yt in e}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function wi(e){if(!e||!e.options)throw Q("App Configuration Object");if(!e.name)throw Q("App Name");const t=["projectId","apiKey","appId","messagingSenderId"],{options:n}=e;for(const r of t)if(!n[r])throw Q(r);return{appName:e.name,projectId:n.projectId,apiKey:n.apiKey,appId:n.appId,senderId:n.messagingSenderId}}function Q(e){return f.create("missing-app-config-values",{valueName:e})}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */class yi{constructor(t,n,r){this.deliveryMetricsExportedToBigQueryEnabled=!1,this.onBackgroundMessageHandler=null,this.onMessageHandler=null,this.logEvents=[],this.isLogServiceStarted=!1;const i=wi(t);this.firebaseDependencies={app:t,appConfig:i,installations:n,analyticsProvider:r}}_delete(){return Promise.resolve()}}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function _t(e){try{e.swRegistration=await navigator.serviceWorker.register(zr,{scope:qr}),e.swRegistration.update().catch(()=>{}),await Ii(e.swRegistration)}catch(t){throw f.create("failed-service-worker-registration",{browserErrorMessage:t==null?void 0:t.message})}}async function Ii(e){return new Promise((t,n)=>{const r=setTimeout(()=>n(new Error(`Service worker not registered after ${ke} ms`)),ke),i=e.installing||e.waiting;e.active?(clearTimeout(r),t()):i?i.onstatechange=o=>{var a;((a=o.target)==null?void 0:a.state)==="activated"&&(i.onstatechange=null,clearTimeout(r),t())}:(clearTimeout(r),n(new Error("No incoming service worker found.")))})}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function Ei(e,t){if(!t&&!e.swRegistration&&await _t(e),!(!t&&e.swRegistration)){if(!(t instanceof ServiceWorkerRegistration))throw f.create("invalid-sw-registration");e.swRegistration=t}}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function Si(e,t){t?e.vapidKey=t:e.vapidKey||(e.vapidKey=wt)}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function Tt(e,t){if(!navigator)throw f.create("only-available-in-window");if(Notification.permission==="default"&&await Notification.requestPermission(),Notification.permission!=="granted")throw f.create("permission-blocked");return await Si(e,t==null?void 0:t.vapidKey),await Ei(e,t==null?void 0:t.serviceWorkerRegistration),ui(e)}/**
 * @license
 * Copyright 2019 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function _i(e,t,n){const r=Ti(t);(await e.firebaseDependencies.analyticsProvider.get()).logEvent(r,{message_id:n[yt],message_name:n[Jr],message_time:n[Yr],message_device_time:Math.floor(Date.now()/1e3)})}function Ti(e){switch(e){case M.NOTIFICATION_CLICKED:return"notification_open";case M.PUSH_RECEIVED:return"notification_foreground";default:throw new Error}}/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function vi(e,t){const n=t.data;if(!n.isFirebaseMessaging)return;e.onMessageHandler&&n.messageType===M.PUSH_RECEIVED&&(typeof e.onMessageHandler=="function"?e.onMessageHandler(Me(n)):e.onMessageHandler.next(Me(n)));const r=n.data;bi(r)&&r[Xr]==="1"&&await _i(e,n.messageType,r)}const Be="@firebase/messaging",$e="0.12.25";/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */const Ai=e=>{const t=new yi(e.getProvider("app").getImmediate(),e.getProvider("installations-internal").getImmediate(),e.getProvider("analytics-internal"));return navigator.serviceWorker.addEventListener("message",n=>vi(t,n)),t},Di=e=>{const t=e.getProvider("messaging").getImmediate();return{getToken:r=>Tt(t,r)}};function Ci(){I(new w("messaging",Ai,"PUBLIC")),I(new w("messaging-internal",Di,"PRIVATE")),h(Be,$e),h(Be,$e,"esm2020")}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function vt(){try{await je()}catch{return!1}return typeof window<"u"&&Ve()&&Ft()&&"serviceWorker"in navigator&&"PushManager"in window&&"Notification"in window&&"fetch"in window&&ServiceWorkerRegistration.prototype.hasOwnProperty("showNotification")&&PushSubscription.prototype.hasOwnProperty("getKey")}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */async function ki(e){if(!navigator)throw f.create("only-available-in-window");return e.swRegistration||await _t(e),di(e)}/**
 * @license
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function Oi(e,t){if(!navigator)throw f.create("only-available-in-window");return e.onMessageHandler=t,()=>{e.onMessageHandler=null}}/**
 * @license
 * Copyright 2017 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */function Ni(e=Ye()){return vt().then(t=>{if(!t)throw f.create("unsupported-browser")},t=>{throw f.create("indexed-db-unsupported")}),B(x(e),"messaging").getImmediate()}async function Ri(e,t){return e=x(e),Tt(e,t)}function Mi(e){return e=x(e),ki(e)}function Bi(e,t){return e=x(e),Oi(e,t)}Ci();const Pi=Object.freeze(Object.defineProperty({__proto__:null,deleteToken:Mi,getMessaging:Ni,getToken:Ri,isSupported:vt,onMessage:Bi},Symbol.toStringTag,{value:"Module"}));export{Pi as a,$i as i};
//# sourceMappingURL=firebase-CL8WnIvY.js.map
