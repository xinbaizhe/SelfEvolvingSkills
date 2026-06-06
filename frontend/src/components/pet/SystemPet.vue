<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue'
import { api } from '../../api/tauri'
import type { ApiResponse } from '../../api/skills'
import { generateDailyReport, fetchGenerationStatus, fetchDailyReport, fetchSystemInfo, fetchLlmConfig } from '../../api/admin'
import { getSystemMonitor, getDatabaseInfo } from '../../api/system'
import { fetchSummary } from '../../api/stats'

type Species='cat'|'dog'|'rabbit'|'fox'|'bird'|'panda'
interface Breed{breed:string;c:string;c2:string;belly:string;dark:string;ear:string;eye:string;nose:string;desc:string}
const BREEDS:Record<Species,Breed[]>={
cat:[
  {breed:'橘猫',c:'#e89840',c2:'#d08030',belly:'#fce8d0',dark:'#984810',ear:'#e8a0a0',eye:'#80d040',nose:'#e88888',desc:'中华田园橘'},
  {breed:'黑猫',c:'#3a3c42',c2:'#2a2c32',belly:'#585a60',dark:'#121418',ear:'#6a4a4a',eye:'#f0d040',nose:'#4a3a3a',desc:'神秘优雅月光猫'},
  {breed:'白猫',c:'#eae6e0',c2:'#dcd8d2',belly:'#f8f6f2',dark:'#a09890',ear:'#f0c8c0',eye:'#48a8f0',nose:'#f0b8b0',desc:'雪白毛茸茸'},
],
dog:[
  {breed:'金毛',c:'#d8b070',c2:'#c89858',belly:'#f5e8d0',dark:'#785018',ear:'#c09060',eye:'#5a4830',nose:'#1a1816',desc:'阳光大金毛'},
  {breed:'哈士奇',c:'#707880',c2:'#5a6268',belly:'#e0e2e6',dark:'#383c42',ear:'#5a5a60',eye:'#50a8f0',nose:'#18181a',desc:'二哈本哈'},
],
rabbit:[{breed:'垂耳兔',c:'#c8bab0',c2:'#b0a298',belly:'#f0e8e0',dark:'#887060',ear:'#e0b8b0',eye:'#684030',nose:'#e89898',desc:'乖巧小兔'}],
fox:[{breed:'赤狐',c:'#e86818',c2:'#d05008',belly:'#fce0c0',dark:'#482008',ear:'#8a3830',eye:'#f8d040',nose:'#1a0808',desc:'聪明红狐狸'}],
bird:[{breed:'虎皮鹦鹉',c:'#38c868',c2:'#20a850',belly:'#c0f8d8',dark:'#186838',ear:'#0000',eye:'#282828',nose:'#f08020',desc:'绿羽毛小鹦鹉'}],
panda:[{breed:'大熊猫',c:'#f2f0ee',c2:'#e4e2e0',belly:'#faf8f6',dark:'#282828',ear:'#282828',eye:'#504030',nose:'#1a1a1a',desc:'国宝滚滚'}],
}
const rnd=<T>(a:readonly T[]):T=>a[Math.floor(Math.random()*a.length)]
const species=ref<Species>(rnd(['cat','dog','rabbit','fox','bird','panda']))
const b=shallowRef(rnd(BREEDS[species.value]))
const petName=ref(b.value.breed+rnd(['','酱','君','宝贝','小可爱']))

// Movement
const x=ref(400),y=ref(300);let tx=400,ty=300,vx=0,vy=0,fs=0
let af=0,lt=0,it=0,st=0;const facing=ref<'l'|'r'>('r')
type Mood='walk'|'idle'|'jump'|'sleep'|'chat'|'sit'|'spin'|'hop'|'dance'
const mood=ref<Mood>('idle');const sq=ref(1),fl=ref(0),eo=ref({x:0,y:0})
const walkPhase=ref(0);const spinCount=ref(0);let danceBeat=0
function pt(){tx=80+Math.random()*(innerWidth-160);ty=80+Math.random()*(innerHeight-160);mood.value='walk';it=0;st=0}
function doJump(dur=700){mood.value='jump';st=0;spinCount.value=0;setTimeout(()=>{if(mood.value==='jump')mood.value='idle'},dur)}
function doSpin(){mood.value='spin';spinCount.value=0;st=0}
function doHop(){mood.value='hop';st=0;setTimeout(()=>{if(mood.value==='hop')mood.value='idle'},400)}
function doDance(){mood.value='dance';danceBeat=0;st=0;setTimeout(()=>{if(mood.value==='dance')mood.value='idle'},3000+Math.random()*2000)}

// Drag
const drag=ref(false);let dsx=0,dsy=0,psx=0,psy=0,dm=false
function ds(e:MouseEvent){if(e.button!==0)return;drag.value=true;dm=false;dsx=e.clientX;dsy=e.clientY;psx=x.value;psy=y.value;mood.value='idle';e.preventDefault()}
function dmv(e:MouseEvent){if(!drag.value)return;const a=e.clientX-dsx,b=e.clientY-dsy;if(Math.abs(a)>3||Math.abs(b)>3)dm=true;if(dm){x.value=Math.max(50,Math.min(innerWidth-50,psx+a));y.value=Math.max(50,Math.min(innerHeight-50,psy+b));tx=x.value;ty=y.value}}
function de(){drag.value=false;it=0;if(!dm)hc()}

// Chat & reminders
const cb=ref(''),cv=ref(false),cl=ref(false),ci=ref(''),cm=ref(false)
let ct:any=null,at:any=null,rt:any=null
function sb(t:string,d=4000){cb.value=t;cv.value=true;if(ct)clearTimeout(ct);ct=setTimeout(()=>{cv.value=false},d)}

const LUNAR:{[k:string]:string}={'2026-02-17':'除夕快乐！🧧','2026-02-18':'新春大吉！🧨','2026-06-19':'端午安康！🎋','2026-09-25':'中秋快乐！🥮'}
function dateKey(d=new Date()):string{return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`}
function isWeekend(d=new Date()):boolean{const w=d.getDay();return w===0||w===6}
function getSmartMessage():string{
  const n=new Date();const h=n.getHours();const m=n.getMinutes();const dk=dateKey(n)
  if(LUNAR[dk])return LUNAR[dk]
  if(isWeekend(n))return rnd(['周末还写代码？休息吧 ☀️','周末还卷？出去走走 🌿'])
  if(h===9&&m<5)return rnd(['上班打卡！元气满满 💼','开始搬砖~今天写什么？'])
  if(h===10&&m<10)return rnd(['记得喝水！每天2升 💧','起来活动一下 🏃'])
  if(h===12&&m<10)return rnd(['午饭时间！🍜','该吃饭啦 🍱'])
  if(h===14&&m<5)return rnd(['下午好！来杯咖啡 ☕','打起精神下午加油'])
  if(h===15&&m<10)return rnd(['摸鱼时间！📱','眼保健操时间 👁️'])
  if(h===18&&m<5)return rnd(['下班！辛苦了 🎉','准时下班好习惯 ✨'])
  if(h>=18&&h<21)return rnd(['还在加班？注意身体 💸','该吃晚饭了 🍲'])
  if(h>=21)return rnd(['九点多了！明天再写 😰','早点休息！🛏️'])
  return rnd(['今天修了几个bug？💪','累了摸摸我充电~','需要生成日报吗？说"日报"就行'])
}
function autoSpeak(){if(cv.value||drag.value||cm.value)return;if(Math.random()<.16)sb(getSmartMessage(),5000)}
async function hc(){
  if(mood.value==='sleep'){mood.value='idle';it=0;sb('呼...睡醒了 ☀️');return}
  if(cm.value)return
  doJump();sb(getSmartMessage())
}
async function dbc(){
  cm.value=!cm.value
  if(cm.value){mood.value='sit';sb(`嗨！我是${petName.value}（${b.value.breed}）~想问什么？也可以生成日报 📊`,5000)}
  else{mood.value='idle';sb('那我先去玩啦~拜拜👋',2500)}
}
async function sc(){
  const msg=ci.value.trim();if(!msg)return;ci.value='';cl.value=true;sb('让我看看...🤔',99999)

  // 1. Check: daily report generation
  if(msg.includes('日报')||msg.includes('总结')||msg.includes('今天做了什么')){
    try{await generateDailyReport(new Date().toISOString().slice(0,10));sb('日报已开始生成！去日报页面查看~📊',5000)}catch{sb('生成失败...检查AI模型配置？',4000)}
    cl.value=false;return
  }

  // 2. Intent matching: system status queries
  const result=await handleSystemQuery(msg)
  if(result){sb(result,6000);cl.value=false;return}

  // 3. Fallback: LLM chat
  try{const r=await api<ApiResponse<{reply:string}>>('POST','/admin/pet/chat',null,{message:msg,pet_type:species.value,pet_name:petName.value});if(r.success&&r.data)sb(r.data.reply,6000);else sb(getSmartMessage(),4000)}catch{sb('AI好像睡着了...💤',4000)}finally{cl.value=false}
}

// ── System query intent matching ──
async function handleSystemQuery(msg:string):Promise<string|null>{
  const m=msg.toLowerCase()
  try{
    // CPU / memory / system status
    if(m.includes('系统状态')||m.includes('cpu')||m.includes('内存')||m.includes('运行状态')||m.includes('系统监控')){
      const r=await getSystemMonitor();if(!r.success||!r.data)return null
      const d=r.data
      return `CPU: ${d.cpu_usage_percent}% (${d.cpu_brand.slice(0,30)}), 内存: ${d.memory.used_mb}MB/${d.memory.total_mb}MB (${d.memory.usage_percent}%), 运行: ${Math.floor(d.uptime_seconds/3600)}小时${Math.floor((d.uptime_seconds%3600)/60)}分`}

    // Database info
    if(m.includes('数据库')||m.includes('多少表')||m.includes('数据量')||m.includes('db')){
      const r=await getDatabaseInfo();if(!r.success||!r.data)return null
      const d=r.data;const tables=Object.values(d.table_counts).reduce((a:number,b:number)=>a+b,0)
      return `数据库 ${d.db_size_mb}MB, ${Object.keys(d.table_counts).length}张表, 共${tables.toLocaleString()}条记录`}

    // System info / uptime
    if(m.includes('系统信息')||m.includes('运行了多久')||m.includes('运行时长')||m.includes('版本')){
      const r=await fetchSystemInfo();if(!r.success||!r.data)return null
      const d=r.data as any
      return `Skills ${d.total_skills}个, Agents ${d.total_agents}个, 会话 ${d.total_sessions}条`}

    // LLM model config
    if(m.includes('模型配置')||m.includes('用的是')&&m.includes('模型')||m.includes('什么模型')){
      const r=await fetchLlmConfig();if(!r.success||!r.data)return null
      const d=r.data as any
      return d.enabled?`当前模型: ${d.provider||'自定义'} / ${d.model}, API: ${d.base_url}`:'模型未启用，去"资源与配置"页面配置吧~'}

    // Stats summary
    if(m.includes('统计')||m.includes('概览')||m.includes('数据总览')){
      const r=await fetchSummary();if(!r.success||!r.data)return null
      const d=r.data as any
      return `Skills ${d.total_skills||0}个, Agents ${d.total_agents||0}个, 会话 ${d.total_sessions||0}个, 使用记录 ${d.total_usage||0}条`}

    // Skills count
    if(m.includes('多少skill')||m.includes('多少个skill')||m.includes('skills数量')||m.includes('技能数量')){
      const r=await fetchSummary();if(!r.success||!r.data)return null
      const d=r.data as any
      return `共有 ${d.total_skills||0} 个 Skills, ${d.total_agents||0} 个 Agents~`}

  }catch{return null}
  return null
}

function anim(t:number){const d=Math.min((t-lt)/1000,.1);lt=t;fl.value=Math.sin(t*.0025)*2;st+=d*1000;it+=d*1000
  if(drag.value){af=requestAnimationFrame(anim);walkPhase.value=0;sq.value=1;return}
  if(mood.value==='sleep')sq.value=1+Math.sin(t*.0018)*.02
  else if(mood.value==='jump'){const p=(st%700)/700;sq.value=1-Math.sin(p*Math.PI)*.25;walkPhase.value=0;if(st>700){mood.value='idle';st=0;sq.value=1}}
  else if(mood.value==='hop'){const p=(st%400)/400;sq.value=1-Math.sin(p*Math.PI)*.15;walkPhase.value=0;if(st>400){mood.value='idle';st=0;sq.value=1}}
  else if(mood.value==='dance'){
    // Dance: bounce to a 150bpm beat (2.5 beats/sec), alternate sides
    danceBeat+=d*4.2
    const beat=Math.sin(danceBeat*Math.PI)
    sq.value=1+Math.abs(beat)*.1
    fl.value=beat*8
    if(danceBeat>1){facing.value=facing.value==='r'?'l':'r';danceBeat-=1}
    walkPhase.value=(beat+1)/2
    if(st>3000+Math.random()*2000){mood.value='idle';st=0;sq.value=1;fl.value=0;walkPhase.value=0}}
  else if(mood.value==='spin'){
    sq.value=1+Math.sin(t*.02)*.04
    spinCount.value+=d*8
    if(spinCount.value>2){facing.value=facing.value==='r'?'l':'r';spinCount.value-=2}
    if(st>800){mood.value='idle';st=0;spinCount.value=0;sq.value=1}}
  else if(mood.value==='walk'){
    const a=tx-x.value,b=ty-y.value,di=Math.sqrt(a*a+b*b)
    if(di<10){mood.value='idle';st=0;walkPhase.value=0}
    else{
      const s=80+Math.sin(t*.004)*12;vx+=(a/di*s-vx)*3.5*d;vy+=(b/di*s-vy)*3.5*d
      x.value+=vx*d;y.value+=vy*d
      if(Math.abs(vx)>3)facing.value=vx>0?'r':'l'
      fs+=d*6;walkPhase.value=(Math.sin(fs)+1)/2
      sq.value=1+Math.abs(Math.cos(fs))*.02
      // random hop while walking
      if(Math.random()<.004){doHop()}
    }
  }else{walkPhase.value=0;sq.value=1+Math.sin(t*.003)*.008;eo.value={x:Math.sin(t*.002)*2,y:Math.cos(t*.003)*1.5}
    // More frequent idle actions (every 3-8 seconds)
    if(it>3000+Math.random()*5000&&mood.value!=='sit'&&mood.value!=='chat'){
      const r=Math.random()
      if(r<.20){doJump(500+Math.random()*400)}        // 20% jump
      else if(r<.35){mood.value='sit';it=0}            // 15% sit
      else if(r<.45){doDance()}                         // 10% dance! 💃
      else if(r<.55){doSpin()}                          // 10% happy spin
      else if(r<.65){doHop()}                           // 10% little hop
      else pt()                                         // 35% walk to new spot
    }
    if(mood.value==='sit'&&it>8000+Math.random()*6000){mood.value='idle';it=0}
  }
  if(x.value<50||x.value>innerWidth-50||y.value<50||y.value>innerHeight-50)pt()
  af=requestAnimationFrame(anim)}

onMounted(()=>{lt=performance.now();af=requestAnimationFrame(anim);setTimeout(()=>pt(),2000);addEventListener('mousemove',dmv);addEventListener('mouseup',de)
  at=setInterval(autoSpeak,35000+Math.random()*30000)
  rt=setInterval(()=>{const h=new Date().getHours();const m=new Date().getMinutes();for(const[hh,mm]of[[9,0],[12,0],[14,0],[15,0],[18,0],[21,0]]){if(h===hh&&m>=mm&&m<mm+3){sb(getSmartMessage(),6000);break}}},60000)})
onUnmounted(()=>{cancelAnimationFrame(af);removeEventListener('mousemove',dmv);removeEventListener('mouseup',de);if(ct)clearTimeout(ct);if(at)clearInterval(at);if(rt)clearInterval(rt)})

// SVG viewBox sizes per species
const svgViewBox=computed(()=>{
  if(species.value==='bird')return '0 0 80 90'
  if(species.value==='rabbit')return '0 0 90 110'
  if(species.value==='panda')return '0 0 110 100'
  return '0 0 100 90'
})

// Walking limb rotation
const walkRot=computed(()=>{
  const p=walkPhase.value
  return {
    bl: (p<.5?p*40-10:(1-p)*40-10),
    br: (p<.5?(1-p)*40-10:p*40-10),
    fl: (p<.5?(1-p)*40-10:p*40-10),
    fr: (p<.5?p*40-10:(1-p)*40-10),
  }
})
</script>

<template>
<div class="sp" :class="[`sp-${species}`,`f-${facing}`,`m-${mood}`]"
  :style="{left:x+'px',top:y+'px',transform:`translate(-50%,-50%) scaleX(${facing==='l'?-1:1}) scale(${sq}) translateY(${fl}px)`}"
  @mousedown="ds" @dblclick.stop="dbc">

  <div class="gs"></div>

  <!-- ─── CAT ─── -->
  <svg v-if="species==='cat'" :viewBox="svgViewBox" class="pet-svg" xmlns="http://www.w3.org/2000/svg">
    <defs>
      <radialGradient id="catBody" cx="45%" cy="30%"><stop offset="0%" :stop-color="b.c"/><stop offset="55%" :stop-color="b.c2"/><stop offset="100%" :stop-color="b.dark"/></radialGradient>
      <radialGradient id="catBelly" cx="50%" cy="60%"><stop offset="0%" :stop-color="b.belly" stop-opacity=".8"/><stop offset="100%" stop-color="transparent"/></radialGradient>
    </defs>

    <!-- TAIL (behind body) -->
    <g transform="translate(28,52)">
      <path d="M0,0 Q-12,-2 -18,-8 Q-22,-12 -20,-14 Q-18,-15 -16,-12 Q-20,-6 0,0" :fill="b.c2">
        <animateTransform attributeName="transform" type="rotate" values="-5 0 0;10 0 0;-3 0 0;10 0 0;-5 0 0" dur="1.3s" repeatCount="indefinite"/>
      </path>
      <path d="M-18,-8 Q-22,-12 -22,-16" :stroke="b.dark" stroke-width="4" fill="none" stroke-linecap="round" opacity=".7">
        <animateTransform attributeName="transform" type="rotate" values="-5 0 0;10 0 0;-3 0 0;10 0 0;-5 0 0" dur="1.3s" repeatCount="indefinite"/>
      </path>
    </g>

    <!-- === BACK LEGS (behind body) === -->
    <g :transform="`translate(35,50) rotate(${walkRot.bl})`" style="transform-origin:0 0">
      <path d="M-10,-8 Q-12,4 -10,14 L-7,14 Q-8,4 -4,-8 Z" :fill="b.c2"/>
      <path d="M-9,12 Q-8,22 -6,30 L-3,30 Q-4,22 -5,12 Z" :fill="b.dark"/>
      <ellipse cx="-4" cy="32" rx="7" ry="4" :fill="b.dark"/>
      <!-- hip joint patch -->
      <ellipse cx="-7" cy="-4" rx="6" ry="5" :fill="b.c2" opacity=".6"/>
    </g>
    <g :transform="`translate(49,50) rotate(${walkRot.br})`" style="transform-origin:0 0">
      <path d="M-10,-8 Q-12,4 -10,14 L-7,14 Q-8,4 -4,-8 Z" :fill="b.c2"/>
      <path d="M-9,12 Q-8,22 -6,30 L-3,30 Q-4,22 -5,12 Z" :fill="b.dark"/>
      <ellipse cx="-4" cy="32" rx="7" ry="4" :fill="b.dark"/>
      <ellipse cx="-7" cy="-4" rx="6" ry="5" :fill="b.c2" opacity=".6"/>
    </g>

    <!-- === FRONT LEGS (behind body) === -->
    <g :transform="`translate(34,46) rotate(${walkRot.fl})`" style="transform-origin:0 0">
      <path d="M-8,-6 Q-10,6 -8,16 L-5,16 Q-6,6 -2,-6 Z" :fill="b.c2"/>
      <path d="M-7,14 Q-6,24 -4,32 L-2,32 Q-3,24 -4,14 Z" :fill="b.dark"/>
      <ellipse cx="-3" cy="34" rx="7" ry="4" :fill="b.dark"/>
      <!-- shoulder patch -->
      <ellipse cx="-5" cy="-2" rx="5" ry="4" :fill="b.c2" opacity=".5"/>
    </g>
    <g :transform="`translate(46,46) rotate(${walkRot.fr})`" style="transform-origin:0 0">
      <path d="M-8,-6 Q-10,6 -8,16 L-5,16 Q-6,6 -2,-6 Z" :fill="b.c2"/>
      <path d="M-7,14 Q-6,24 -4,32 L-2,32 Q-3,24 -4,14 Z" :fill="b.dark"/>
      <ellipse cx="-3" cy="34" rx="7" ry="4" :fill="b.dark"/>
      <ellipse cx="-5" cy="-2" rx="5" ry="4" :fill="b.c2" opacity=".5"/>
    </g>

    <!-- === TORSO (overlaps legs) === -->
    <ellipse cx="40" cy="44" rx="26" ry="19" fill="url(#catBody)"/>
    <ellipse cx="40" cy="52" rx="15" ry="10" fill="url(#catBelly)"/>

    <!-- Stripes -->
    <g opacity=".13" :stroke="b.dark" stroke-width="3" stroke-linecap="round">
      <path d="M27,40 Q32,42 36,40"/><path d="M29,46 Q33,48 37,46"/>
      <path d="M53,40 Q48,42 44,40"/><path d="M51,46 Q47,48 43,46"/>
    </g>

    <!-- === NECK + HEAD === -->
    <path d="M33,30 Q34,22 40,18 Q46,22 47,30 Q44,36 40,38 Q36,36 33,30 Z" fill="url(#catBody)"/>
    <circle cx="40" cy="16" r="16" fill="url(#catBody)"/>
    <ellipse cx="40" cy="22" rx="10" ry="7" :fill="b.belly" opacity=".25"/>

    <!-- Ears -->
    <polygon points="26,6 22,-4 32,3" :fill="b.c2"/><polygon points="27,5 24,0 29,4" :fill="b.ear" opacity=".5"/>
    <polygon points="54,6 58,-4 48,3" :fill="b.c2"/><polygon points="53,5 56,0 51,4" :fill="b.ear" opacity=".5"/>

    <!-- Face -->
    <ellipse cx="34" cy="15" rx="4.5" ry="5.5" fill="#fff"/><ellipse cx="34" cy="16" rx="3.2" ry="4.2" :fill="b.eye"/><circle cx="34" cy="16" r="1.8" :fill="b.dark"/><circle cx="33" cy="14.5" r="1.3" fill="#fff"/>
    <ellipse cx="46" cy="15" rx="4.5" ry="5.5" fill="#fff"/><ellipse cx="46" cy="16" rx="3.2" ry="4.2" :fill="b.eye"/><circle cx="46" cy="16" r="1.8" :fill="b.dark"/><circle cx="45" cy="14.5" r="1.3" fill="#fff"/>
    <ellipse cx="40" cy="21" rx="2.5" ry="2" :fill="b.nose"/>
    <path d="M37,24 Q40,28 40,24 Q40,28 43,24" stroke="rgba(0,0,0,.18)" stroke-width="1" fill="none"/>
    <g stroke="rgba(0,0,0,.1)" stroke-width=".7">
      <line x1="16" y1="18" x2="28" y2="20"/><line x1="16" y1="21" x2="28" y2="22"/><line x1="17" y1="24" x2="28" y2="24"/>
      <line x1="64" y1="18" x2="52" y2="20"/><line x1="64" y1="21" x2="52" y2="22"/><line x1="63" y1="24" x2="52" y2="24"/>
    </g>
  </svg>

  <!-- ─── DOG ─── -->
  <svg v-else-if="species==='dog'" :viewBox="svgViewBox" class="pet-svg" xmlns="http://www.w3.org/2000/svg">
    <defs>
      <radialGradient id="dogBody" cx="45%" cy="30%"><stop offset="0%" :stop-color="b.c"/><stop offset="55%" :stop-color="b.c2"/><stop offset="100%" :stop-color="b.dark"/></radialGradient>
    </defs>

    <!-- TAIL -->
    <g transform="translate(28,46)">
      <path d="M0,0 Q-14,0 -18,-4 Q-20,-6 -18,-8 Q-15,-7 -14,-5 Q-16,0 0,0" :fill="b.c2">
        <animateTransform attributeName="transform" type="rotate" values="-8 0 0;16 0 0;-4 0 0;16 0 0;-8 0 0" dur=".45s" repeatCount="indefinite"/>
      </path>
    </g>

    <!-- BACK LEGS (behind body) -->
    <g :transform="`translate(33,46) rotate(${walkRot.bl})`" style="transform-origin:0 0">
      <path d="M-12,-6 Q-14,8 -12,18 L-8,18 Q-9,8 -5,-6 Z" :fill="b.c2"/>
      <path d="M-11,16 Q-10,26 -8,34 L-5,34 Q-6,26 -7,16 Z" :fill="b.dark"/>
      <ellipse cx="-6" cy="36" rx="8" ry="5" :fill="b.dark"/>
      <ellipse cx="-8" cy="-2" rx="7" ry="6" :fill="b.c2" opacity=".5"/>
    </g>
    <g :transform="`translate(49,46) rotate(${walkRot.br})`" style="transform-origin:0 0">
      <path d="M-12,-6 Q-14,8 -12,18 L-8,18 Q-9,8 -5,-6 Z" :fill="b.c2"/>
      <path d="M-11,16 Q-10,26 -8,34 L-5,34 Q-6,26 -7,16 Z" :fill="b.dark"/>
      <ellipse cx="-6" cy="36" rx="8" ry="5" :fill="b.dark"/>
      <ellipse cx="-8" cy="-2" rx="7" ry="6" :fill="b.c2" opacity=".5"/>
    </g>

    <!-- FRONT LEGS (behind body) -->
    <g :transform="`translate(32,44) rotate(${walkRot.fl})`" style="transform-origin:0 0">
      <path d="M-10,-6 Q-12,8 -10,18 L-7,18 Q-8,8 -3,-6 Z" :fill="b.c2"/>
      <path d="M-9,16 Q-8,26 -6,34 L-3,34 Q-4,26 -5,16 Z" :fill="b.dark"/>
      <ellipse cx="-4" cy="36" rx="8" ry="5" :fill="b.dark"/>
      <ellipse cx="-6" cy="-2" rx="6" ry="5" :fill="b.c2" opacity=".4"/>
    </g>
    <g :transform="`translate(48,44) rotate(${walkRot.fr})`" style="transform-origin:0 0">
      <path d="M-10,-6 Q-12,8 -10,18 L-7,18 Q-8,8 -3,-6 Z" :fill="b.c2"/>
      <path d="M-9,16 Q-8,26 -6,34 L-3,34 Q-4,26 -5,16 Z" :fill="b.dark"/>
      <ellipse cx="-4" cy="36" rx="8" ry="5" :fill="b.dark"/>
      <ellipse cx="-6" cy="-2" rx="6" ry="5" :fill="b.c2" opacity=".4"/>
    </g>

    <!-- TORSO (overlaps legs) -->
    <ellipse cx="40" cy="44" rx="28" ry="20" fill="url(#dogBody)"/>
    <ellipse cx="40" cy="52" rx="17" ry="11" :fill="b.belly" opacity=".5"/>

    <!-- NECK + HEAD -->
    <path d="M32,30 Q33,20 40,16 Q47,20 48,30 Q44,36 40,38 Q36,36 32,30 Z" fill="url(#dogBody)"/>
    <ellipse cx="40" cy="18" rx="18" ry="17" fill="url(#dogBody)"/>
    <ellipse cx="40" cy="30" rx="12" ry="9" :fill="b.belly" opacity=".5"/>

    <!-- Ears -->
    <ellipse cx="24" cy="8" rx="7" ry="13" :fill="b.c2" transform="rotate(-10 24 8)"/>
    <ellipse cx="56" cy="8" rx="7" ry="13" :fill="b.c2" transform="rotate(10 56 8)"/>

    <!-- Face -->
    <circle cx="33" cy="18" r="4.5" fill="#fff"/><circle cx="33" cy="19" r="3.2" :fill="b.eye"/><circle cx="33" cy="19" r="1.8" :fill="b.dark"/><circle cx="32" cy="17.5" r="1.2" fill="#fff"/>
    <circle cx="47" cy="18" r="4.5" fill="#fff"/><circle cx="47" cy="19" r="3.2" :fill="b.eye"/><circle cx="47" cy="19" r="1.8" :fill="b.dark"/><circle cx="46" cy="17.5" r="1.2" fill="#fff"/>
    <ellipse cx="40" cy="26" rx="5" ry="3.5" :fill="b.nose"/>
    <ellipse cx="38.5" cy="25" rx="2" ry="1.3" fill="rgba(255,255,255,.2)"/>
    <path d="M36,30 Q40,35 40,30 Q40,35 44,30" stroke="rgba(0,0,0,.18)" stroke-width="1.2" fill="none"/>
  </svg>

  <!-- ─── RABBIT ─── -->
  <svg v-else-if="species==='rabbit'" :viewBox="svgViewBox" class="pet-svg" xmlns="http://www.w3.org/2000/svg">
    <defs><radialGradient id="rbBody" cx="42%" cy="35%"><stop offset="0%" :stop-color="b.c"/><stop offset="60%" :stop-color="b.c2"/><stop offset="100%" :stop-color="b.dark"/></radialGradient></defs>
    <!-- Tail -->
    <circle cx="20" cy="58" r="7" :fill="b.belly" opacity=".8"/>
    <!-- Back legs - large, connected -->
    <ellipse cx="32" cy="66" rx="12" ry="16" :fill="b.c2" transform="rotate(-18 32 66)"/>
    <ellipse cx="32" cy="78" rx="10" ry="6" :fill="b.dark" transform="rotate(-10 32 78)"/>
    <ellipse cx="48" cy="66" rx="12" ry="16" :fill="b.c2" transform="rotate(18 48 66)"/>
    <ellipse cx="48" cy="78" rx="10" ry="6" :fill="b.dark" transform="rotate(10 48 78)"/>
    <!-- Torso -->
    <path d="M24,40 Q20,48 22,58 Q26,64 40,66 Q54,64 58,58 Q60,48 56,40 Q48,34 40,33 Q32,34 24,40 Z" fill="url(#rbBody)"/>
    <ellipse cx="40" cy="58" rx="14" ry="10" :fill="b.belly" opacity=".6"/>
    <!-- Front legs -->
    <g :transform="`translate(30,50) rotate(${walkRot.fl/2})`" style="transform-origin:30px 50px">
      <path d="M-3,0 Q-5,10 -4,18 L-2,18 Q-1,10 1,0 Z" :fill="b.c2"/>
      <ellipse cx="-3" cy="20" rx="5" ry="3" :fill="b.dark"/>
    </g>
    <g :transform="`translate(46,50) rotate(${walkRot.fr/2})`" style="transform-origin:46px 50px">
      <path d="M-3,0 Q-5,10 -4,18 L-2,18 Q-1,10 1,0 Z" :fill="b.c2"/>
      <ellipse cx="-3" cy="20" rx="5" ry="3" :fill="b.dark"/>
    </g>
    <!-- Neck + Head -->
    <path d="M34,36 Q35,26 40,24 Q45,26 46,36 Q44,40 40,41 Q36,40 34,36 Z" fill="url(#rbBody)"/>
    <ellipse cx="40" cy="24" rx="15" ry="15" fill="url(#rbBody)"/>
    <!-- Ears (very long, flowing from head) -->
    <ellipse cx="31" cy="0" rx="5" ry="18" :fill="b.c2"/><ellipse cx="31" cy="0" rx="3" ry="13" :fill="b.ear" opacity=".5"/>
    <ellipse cx="49" cy="0" rx="5" ry="18" :fill="b.c2"/><ellipse cx="49" cy="0" rx="3" ry="13" :fill="b.ear" opacity=".5"/>
    <!-- Face -->
    <ellipse cx="40" cy="28" rx="11" ry="7" :fill="b.belly" opacity=".4"/>
    <circle cx="35" cy="22" r="4" fill="#fff"/><circle cx="35" cy="23" r="2.8" :fill="b.eye"/><circle cx="35" cy="23" r="1.5" :fill="b.dark"/><circle cx="34" cy="22" r="1" fill="#fff"/>
    <circle cx="45" cy="22" r="4" fill="#fff"/><circle cx="45" cy="23" r="2.8" :fill="b.eye"/><circle cx="45" cy="23" r="1.5" :fill="b.dark"/><circle cx="44" cy="22" r="1" fill="#fff"/>
    <ellipse cx="40" cy="27" rx="2.5" ry="2" :fill="b.nose"/>
    <path d="M37,30 Q40,33 40,30 Q40,33 43,30" stroke="rgba(0,0,0,.14)" stroke-width="1" fill="none"/>
    <g stroke="rgba(0,0,0,.09)" stroke-width=".6">
      <line x1="18" y1="26" x2="28" y2="28"/><line x1="18" y1="29" x2="28" y2="29"/>
      <line x1="62" y1="26" x2="52" y2="28"/><line x1="62" y1="29" x2="52" y2="29"/>
    </g>
  </svg>

  <!-- ─── FOX ─── -->
  <svg v-else-if="species==='fox'" :viewBox="svgViewBox" class="pet-svg" xmlns="http://www.w3.org/2000/svg">
    <defs><radialGradient id="fxBody" cx="45%" cy="30%"><stop offset="0%" :stop-color="b.c"/><stop offset="55%" :stop-color="b.c2"/><stop offset="100%" :stop-color="b.dark"/></radialGradient></defs>
    <!-- Bushy tail -->
    <path d="M24,48 Q8,44 2,36 Q-2,30 4,26 Q8,24 12,26 Q6,34 26,46" :fill="b.c2"/>
    <ellipse cx="5" cy="27" rx="5" ry="3" :fill="b.belly" opacity=".8"/>
    <!-- Back legs -->
    <g :transform="`translate(34,46) rotate(${walkRot.bl})`" style="transform-origin:34px 46px">
      <path d="M-4,0 Q-6,12 -5,20 L-3,20 Q-2,12 1,0 Z" :fill="b.c2"/>
      <path d="M-4,18 Q-3,26 -2,32 L-1,32 Q-2,26 -1,18 Z" :fill="b.dark"/>
      <ellipse cx="-2" cy="34" rx="6" ry="3" :fill="b.dark"/>
    </g>
    <g :transform="`translate(48,46) rotate(${walkRot.br})`" style="transform-origin:48px 46px">
      <path d="M-4,0 Q-6,12 -5,20 L-3,20 Q-2,12 1,0 Z" :fill="b.c2"/>
      <path d="M-4,18 Q-3,26 -2,32 L-1,32 Q-2,26 -1,18 Z" :fill="b.dark"/>
      <ellipse cx="-2" cy="34" rx="6" ry="3" :fill="b.dark"/>
    </g>
    <!-- Torso -->
    <path d="M27,38 Q23,44 24,54 Q28,62 40,64 Q52,62 56,54 Q57,44 53,38 Q48,32 40,30 Q32,32 27,38 Z" fill="url(#fxBody)"/>
    <ellipse cx="40" cy="54" rx="14" ry="9" :fill="b.belly" opacity=".5"/>
    <!-- Front legs -->
    <g :transform="`translate(32,46) rotate(${walkRot.fl})`" style="transform-origin:32px 46px">
      <path d="M-4,0 Q-6,12 -5,20 L-3,20 Q-2,12 1,0 Z" :fill="b.c2"/>
      <path d="M-4,18 Q-3,26 -2,32 L-1,32 Q-2,26 -1,18 Z" :fill="b.dark"/>
      <ellipse cx="-2" cy="34" rx="6" ry="3" :fill="b.dark"/>
    </g>
    <g :transform="`translate(48,46) rotate(${walkRot.fr})`" style="transform-origin:48px 46px">
      <path d="M-4,0 Q-6,12 -5,20 L-3,20 Q-2,12 1,0 Z" :fill="b.c2"/>
      <path d="M-4,18 Q-3,26 -2,32 L-1,32 Q-2,26 -1,18 Z" :fill="b.dark"/>
      <ellipse cx="-2" cy="34" rx="6" ry="3" :fill="b.dark"/>
    </g>
    <!-- Neck + Head (pointy fox face) -->
    <path d="M32,32 Q33,18 40,14 Q47,18 48,32 Q44,36 40,37 Q36,36 32,32 Z" fill="url(#fxBody)"/>
    <path d="M25,22 Q40,10 55,22 Q48,30 40,31 Q32,30 25,22 Z" fill="url(#fxBody)"/>
    <!-- Ears -->
    <polygon points="30,16 24,-2 34,10" :fill="b.c2"/><polygon points="31,14 27,2 32,11" :fill="b.ear" opacity=".5"/>
    <polygon points="50,16 56,-2 46,10" :fill="b.c2"/><polygon points="49,14 53,2 48,11" :fill="b.ear" opacity=".5"/>
    <!-- Lighter muzzle -->
    <ellipse cx="40" cy="28" rx="10" ry="6" :fill="b.belly" opacity=".45"/>
    <!-- Eyes -->
    <ellipse cx="33" cy="20" rx="4" ry="5" fill="#fff"/><ellipse cx="33" cy="21" rx="3" ry="4" :fill="b.eye"/><circle cx="33" cy="21" r="1.7" :fill="b.dark"/><circle cx="32" cy="19.5" r="1.2" fill="#fff"/>
    <ellipse cx="47" cy="20" rx="4" ry="5" fill="#fff"/><ellipse cx="47" cy="21" rx="3" ry="4" :fill="b.eye"/><circle cx="47" cy="21" r="1.7" :fill="b.dark"/><circle cx="46" cy="19.5" r="1.2" fill="#fff"/>
    <!-- Nose + mouth -->
    <ellipse cx="40" cy="26" rx="2.5" ry="2" :fill="b.nose"/>
    <path d="M37,29 Q40,33 40,29 Q40,33 43,29" stroke="rgba(0,0,0,.16)" stroke-width="1" fill="none"/>
  </svg>

  <!-- ─── BIRD ─── -->
  <svg v-else-if="species==='bird'" :viewBox="svgViewBox" class="pet-svg" xmlns="http://www.w3.org/2000/svg">
    <defs><radialGradient id="bdBody" cx="38%" cy="30%"><stop offset="0%" :stop-color="b.c"/><stop offset="70%" :stop-color="b.c2"/><stop offset="100%" :stop-color="b.dark"/></radialGradient></defs>
    <!-- Tail -->
    <path d="M32,52 L18,44 L16,50 L28,56" :fill="b.c2"/><path d="M34,54 L20,48 L22,54 L30,58" :fill="b.dark"/>
    <!-- Body + head as one continuous teardrop -->
    <path d="M26,34 Q24,40 26,50 Q30,64 40,66 Q50,64 54,56 Q56,48 52,42 Q48,36 42,34 Q36,30 30,26 Q26,30 26,34 Z" fill="url(#bdBody)"/>
    <ellipse cx="38" cy="46" rx="10" ry="12" :fill="b.belly" opacity=".4"/>
    <!-- Wing -->
    <path d="M46,44 Q56,42 58,52 Q54,58 47,56 Z" :fill="b.c2" opacity=".8"/>
    <!-- Feet -->
    <line x1="35" y1="66" x2="34" y2="76" :stroke="b.dark" stroke-width="2.5" stroke-linecap="round"/>
    <line x1="34" y1="76" x2="30" y2="80" :stroke="b.dark" stroke-width="1.5"/><line x1="34" y1="76" x2="35" y2="81" :stroke="b.dark" stroke-width="1.5"/><line x1="34" y1="76" x2="39" y2="80" :stroke="b.dark" stroke-width="1.5"/>
    <line x1="43" y1="66" x2="44" y2="76" :stroke="b.dark" stroke-width="2.5" stroke-linecap="round"/>
    <line x1="44" y1="76" x2="40" y2="80" :stroke="b.dark" stroke-width="1.5"/><line x1="44" y1="76" x2="45" y2="81" :stroke="b.dark" stroke-width="1.5"/><line x1="44" y1="76" x2="49" y2="80" :stroke="b.dark" stroke-width="1.5"/>
    <!-- Eye -->
    <circle cx="34" cy="30" r="3.5" fill="#fff"/><circle cx="34" cy="31" r="2.5" :fill="b.eye"/><circle cx="34" cy="31" r="1.3" :fill="b.dark"/><circle cx="33" cy="30" r=".8" fill="#fff"/>
    <!-- Beak -->
    <polygon points="26,32 18,36 26,38" :fill="b.nose"/>
    <!-- Crest -->
    <path d="M34,22 Q38,16 42,22" :stroke="b.dark" stroke-width="2" fill="none" opacity=".4"/>
  </svg>

  <!-- ─── PANDA ─── -->
  <svg v-else :viewBox="svgViewBox" class="pet-svg" xmlns="http://www.w3.org/2000/svg">
    <defs><radialGradient id="pdBody" cx="45%" cy="35%"><stop offset="0%" :stop-color="b.c"/><stop offset="70%" :stop-color="b.c2"/><stop offset="100%" :stop-color="b.c2"/></radialGradient></defs>
    <!-- Tail -->
    <circle cx="24" cy="54" r="8" :fill="b.dark"/>
    <!-- Back legs - dark, large -->
    <ellipse cx="32" cy="68" rx="14" ry="18" :fill="b.dark"/><ellipse cx="32" cy="80" rx="10" ry="6" :fill="b.dark"/>
    <ellipse cx="52" cy="68" rx="14" ry="18" :fill="b.dark"/><ellipse cx="52" cy="80" rx="10" ry="6" :fill="b.dark"/>
    <!-- Torso - big round panda body -->
    <path d="M24,38 Q22,48 24,58 Q28,66 42,68 Q56,66 60,58 Q62,48 60,38 Q52,30 42,28 Q32,30 24,38 Z" fill="url(#pdBody)"/>
    <ellipse cx="42" cy="58" rx="20" ry="10" :fill="b.dark" opacity=".25"/>
    <!-- Front legs - dark -->
    <g :transform="`translate(32,50) rotate(${walkRot.fl/2})`" style="transform-origin:32px 50px">
      <path d="M-5,0 Q-8,14 -7,24 L-4,24 Q-3,14 2,0 Z" :fill="b.dark"/>
      <ellipse cx="-5" cy="28" rx="8" ry="5" :fill="b.dark"/>
    </g>
    <g :transform="`translate(50,50) rotate(${walkRot.fr/2})`" style="transform-origin:50px 50px">
      <path d="M-5,0 Q-8,14 -7,24 L-4,24 Q-3,14 2,0 Z" :fill="b.dark"/>
      <ellipse cx="-5" cy="28" rx="8" ry="5" :fill="b.dark"/>
    </g>
    <!-- Neck -->
    <path d="M33,30 Q34,18 42,14 Q50,18 51,30 Q48,36 42,38 Q36,36 33,30 Z" fill="url(#pdBody)"/>
    <!-- Head -->
    <circle cx="42" cy="18" r="20" fill="url(#pdBody)"/>
    <!-- Ears -->
    <circle cx="26" cy="4" r="8" :fill="b.dark"/><circle cx="25" cy="3" r="4" fill="rgba(0,0,0,.15)"/>
    <circle cx="58" cy="4" r="8" :fill="b.dark"/><circle cx="59" cy="3" r="4" fill="rgba(0,0,0,.15)"/>
    <!-- Eye patches -->
    <ellipse cx="32" cy="18" rx="9" ry="8" :fill="b.dark" transform="rotate(-8 32 18)"/>
    <ellipse cx="52" cy="18" rx="9" ry="8" :fill="b.dark" transform="rotate(8 52 18)"/>
    <!-- Eyes -->
    <circle cx="32" cy="18" r="3.5" fill="#fff"/><circle cx="32" cy="19" r="2.3" :fill="b.eye"/><circle cx="32" cy="19" r="1.3" :fill="b.dark"/><circle cx="31" cy="17.5" r=".9" fill="#fff"/>
    <circle cx="52" cy="18" r="3.5" fill="#fff"/><circle cx="52" cy="19" r="2.3" :fill="b.eye"/><circle cx="52" cy="19" r="1.3" :fill="b.dark"/><circle cx="51" cy="17.5" r=".9" fill="#fff"/>
    <!-- Snout -->
    <ellipse cx="42" cy="28" rx="10" ry="8" :fill="b.belly" opacity=".5"/>
    <!-- Nose -->
    <ellipse cx="42" cy="26" rx="4" ry="3" :fill="b.nose"/>
    <path d="M39,30 Q42,34 42,30 Q42,34 45,30" stroke="rgba(0,0,0,.14)" stroke-width="1.2" fill="none"/>
  </svg>

  <!-- Zzz / Name / Chat (same as before) -->
  <div class="zzz-wrap" v-if="mood==='sleep'"><span v-for="i in 3":key="i":style="{animationDelay:i*.4+'s'}">Z</span></div>
  <div class="name-tag" :style="facing==='l'?{transform:'translateX(-50%) scaleX(-1)'}:{}">{{ b.breed }}</div>

  <Transition name="bb"><div class="chat-bubble" v-if="cv" @mousedown.stop :style="facing==='l'?{transform:'translateX(-50%) scaleX(-1)'}:{}">
    <div class="cb-header"><span class="cb-pet-emoji">{{ species==='cat'?'🐱':species==='dog'?'🐶':species==='rabbit'?'🐰':species==='fox'?'🦊':species==='bird'?'🐦':'🐼' }}</span><span class="cb-pet-name">{{ petName }}</span></div>
    <div class="cb-body">{{ cb }}</div><div class="cb-arrow"></div>
  </div></Transition>

  <div class="chat-input-bar" v-if="cm" @mousedown.stop @click.stop :style="facing==='l'?{transform:'translateX(-50%) scaleX(-1)'}:{}">
    <span class="cib-emoji">{{ species==='cat'?'🐱':species==='dog'?'🐶':species==='rabbit'?'🐰':species==='fox'?'🦊':species==='bird'?'🐦':'🐼' }}</span>
    <input v-model="ci" :placeholder="'和'+petName+'聊天...'" @keyup.enter="sc" :disabled="cl" class="cib-input"/>
    <button class="cib-send" @click="sc":disabled="cl">发送</button>
  </div>
</div>
</template>

<style scoped>
.sp{position:fixed;z-index:99999;width:0;height:0;cursor:grab;user-select:none}
.sp:active{cursor:grabbing}.m-sleep{cursor:default}
.gs{position:absolute;bottom:-58px;left:50%;transform:translateX(-50%);width:80px;height:16px;background:radial-gradient(ellipse,rgba(0,0,0,.16) 0%,transparent 75%);border-radius:50%;transition:.3s}
.m-jump .gs{transform:translateX(-50%) scale(.3);opacity:.3}
.pet-svg{position:absolute;left:-55px;top:-85px;width:110px;height:120px;overflow:visible;filter:drop-shadow(0 6px 14px rgba(0,0,0,.18))}
.sp-bird .pet-svg{left:-40px;top:-75px;width:80px;height:100px}
.sp-panda .pet-svg{left:-60px;top:-90px;width:120px;height:130px}

.zzz-wrap{position:absolute;top:-40px;right:-20px;display:flex;flex-direction:column;font-size:12px;font-weight:700;color:#a0a8b8;pointer-events:none}
.zzz-wrap span{animation:z 1.2s ease-out infinite;opacity:0}
@keyframes z{0%{opacity:0;transform:translate(0,0)}30%{opacity:1}100%{opacity:0;transform:translate(6px,-14px)}}

.name-tag{position:absolute;top:-38px;left:50%;transform:translateX(-50%);font-size:10px;font-weight:600;color:var(--c);filter:brightness(.65);pointer-events:none;opacity:0;transition:opacity .3s;white-space:nowrap;text-shadow:0 1px 2px rgba(255,255,255,.85)}
.sp:hover .name-tag{opacity:1}

.chat-bubble{position:absolute;bottom:76px;left:50%;transform:translateX(-50%);width:260px;max-width:300px;background:#fff;color:#1e293b;border-radius:16px;box-shadow:0 4px 24px rgba(0,0,0,.14);text-align:left;word-break:break-word;overflow:hidden}
.cb-header{display:flex;align-items:center;gap:8px;padding:10px 14px 6px;background:linear-gradient(135deg,#f0f9ff,#f8fafc);border-bottom:1px solid #f1f5f9}
.cb-pet-emoji{font-size:18px}.cb-pet-name{font-size:13px;font-weight:700;color:#334155}
.cb-body{padding:10px 14px 12px;font-size:13px;line-height:1.55}
.cb-arrow{position:absolute;bottom:-7px;left:50%;transform:translateX(-50%);width:0;height:0;border-left:8px solid transparent;border-right:8px solid transparent;border-top:8px solid #fff}
.bb-enter-active{transition:all .25s ease}.bb-leave-active{transition:all .18s ease}.bb-enter-from,.bb-leave-to{opacity:0;transform:translateX(-50%) translateY(8px)}

.chat-input-bar{position:absolute;bottom:-58px;left:50%;transform:translateX(-50%);display:flex;align-items:center;gap:6px;background:#fff;padding:6px 8px;border-radius:14px;box-shadow:0 3px 16px rgba(0,0,0,.12);z-index:2}
.cib-emoji{font-size:16px;flex-shrink:0}
.cib-input{border:none;outline:none;padding:6px 4px;font-size:13px;width:180px;background:transparent;color:#1e293b}.cib-input::placeholder{color:#b0b8c4}
.cib-send{padding:6px 16px;border:none;border-radius:8px;background:linear-gradient(135deg,#0ea5e9,#0284c7);color:#fff;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0}.cib-send:disabled{opacity:.5}

.m-sleep .pet-svg{animation:br 2.5s ease-in-out infinite}@keyframes br{0%,100%{transform:scale(1)}50%{transform:scale(1.03)}}
</style>
