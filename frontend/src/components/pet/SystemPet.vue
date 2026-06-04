<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { api } from '../../api/tauri'
import type { ApiResponse } from '../../api/skills'
import { generateDailyReport, fetchGenerationStatus, fetchDailyReport } from '../../api/admin'

// ── Species & Breeds ──
type Species = 'cat' | 'dog' | 'rabbit' | 'fox' | 'bird' | 'panda'

interface BreedInfo {
  breed: string
  color: string        // main fur color
  belly: string        // belly/underside color (lighter)
  dark: string         // dark accent (ears, tail tip, paws)
  eyeColor: string
  desc: string
}

const BREEDS: Record<Species, BreedInfo[]> = {
  cat: [
    { breed:'橘猫',   color:'#f59e0b', belly:'#fef3c7', dark:'#d97706', eyeColor:'#22c55e', desc:'中华田园橘，十橘九胖' },
    { breed:'黑猫',   color:'#334155', belly:'#64748b', dark:'#0f172a', eyeColor:'#fbbf24', desc:'神秘优雅，月光下会发光' },
    { breed:'白猫',   color:'#f8fafc', belly:'#f1f5f9', dark:'#cbd5e1', eyeColor:'#3b82f6', desc:'雪白毛茸茸，像棉花糖' },
    { breed:'暹罗',   color:'#d4a574', belly:'#ede9d0', dark:'#4a3728', eyeColor:'#3b82f6', desc:'挖煤工配色，高贵优雅' },
    { breed:'英短蓝猫',color:'#7a8b9e', belly:'#bcc7d4', dark:'#4a5568', eyeColor:'#f59e0b', desc:'圆脸圆眼呆萌英短' },
    { breed:'三花',   color:'#f8fafc', belly:'#f1f5f9', dark:'#f97316', eyeColor:'#22c55e', desc:'三色花纹招财猫' },
  ],
  dog: [
    { breed:'金毛',   color:'#d4a054', belly:'#f5e6c8', dark:'#8b6914', eyeColor:'#4a3728', desc:'阳光温暖的大金毛' },
    { breed:'哈士奇', color:'#6b7280', belly:'#e5e7eb', dark:'#374151', eyeColor:'#3b82f6', desc:'二哈本哈，精力无限' },
    { breed:'柯基',   color:'#e8943a', belly:'#fde9c8', dark:'#92400e', eyeColor:'#4a3728', desc:'小短腿电动翘臀' },
    { breed:'泰迪',   color:'#8b5e3c', belly:'#c9a882', dark:'#5c3a1e', eyeColor:'#1e293b', desc:'卷毛聪明小可爱' },
    { breed:'柴犬',   color:'#e8a840', belly:'#fef7e8', dark:'#92400e', eyeColor:'#4a3728', desc:'傲娇表情包担当' },
    { breed:'萨摩耶', color:'#f8fafc', belly:'#f1f5f9', dark:'#cbd5e1', eyeColor:'#1e293b', desc:'微笑天使，云朵般笑容' },
  ],
  rabbit: [
    { breed:'垂耳兔', color:'#c4b5a5', belly:'#ede4d8', dark:'#8b7355', eyeColor:'#4a3728', desc:'耳朵软软垂下的乖巧小兔' },
    { breed:'侏儒兔', color:'#f8fafc', belly:'#f1f5f9', dark:'#cbd5e1', eyeColor:'#ef4444', desc:'迷你小团子圆滚滚' },
  ],
  fox: [
    { breed:'赤狐',   color:'#ea580c', belly:'#fed7aa', dark:'#4a1c00', eyeColor:'#fbbf24', desc:'聪明机灵的红狐狸' },
    { breed:'北极狐', color:'#f8fafc', belly:'#f1f5f9', dark:'#94a3b8', eyeColor:'#3b82f6', desc:'雪白蓬松的北极精灵' },
  ],
  bird: [
    { breed:'虎皮鹦鹉',color:'#22c55e', belly:'#bbf7d0', dark:'#166534', eyeColor:'#1e293b', desc:'绿羽毛小鹦鹉爱学话' },
    { breed:'玄凤鹦鹉',color:'#cbd5e1', belly:'#f8fafc', dark:'#fbbf24', eyeColor:'#1e293b', desc:'头顶皇冠会唱歌' },
  ],
  panda: [
    { breed:'大熊猫', color:'#f5f5f4', belly:'#f8fafc', dark:'#1e293b', eyeColor:'#4a3728', desc:'国宝滚滚只会卖萌' },
    { breed:'小熊猫', color:'#d97706', belly:'#fed7aa', dark:'#451a03', eyeColor:'#1e293b', desc:'红毛蓬松尾巴超可爱' },
  ],
}

function rnd<T>(a:readonly T[]):T{return a[Math.floor(Math.random()*a.length)]}
const species=ref<Species>(rnd(['cat','dog','rabbit','fox','bird','panda']))
const b=ref(rnd(BREEDS[species.value]))
const petName=ref(b.value.breed+['','酱','君','宝贝','小可爱'][Math.floor(Math.random()*5)])
const emoji=computed(()=>{const m:Record<Species,string>={cat:'🐱',dog:'🐶',rabbit:'🐰',fox:'🦊',bird:'🐦',panda:'🐼'};return m[species.value]})
function randomizePet(){species.value=rnd(['cat','dog','rabbit','fox','bird','panda']);b.value=rnd(BREEDS[species.value]);petName.value=b.value.breed+['','酱','君','宝贝','小可爱'][Math.floor(Math.random()*5)]}

// ── Movement ──
const x=ref(300),y=ref(400);let tx=300,ty=400,vx=0,vy=0,fs=0
let af=0,lt=0,it=0,st=0;const facing=ref<'l'|'r'>('r')
const mood=ref<'walk'|'idle'|'jump'|'sleep'|'chat'>('idle')
const sq=ref(1),fl=ref(0),eo=ref({x:0,y:0})
function pt(){tx=40+Math.random()*(innerWidth-80);ty=40+Math.random()*(innerHeight-80);mood.value='walk';it=0;st=0}

// ── Drag ──
const drag=ref(false);let dx=0,dy=0,px=0,py=0,dm=false
function ds(e:MouseEvent){if(e.button!==0)return;drag.value=true;dm=false;dx=e.clientX;dy=e.clientY;px=x.value;py=y.value;mood.value='idle';e.preventDefault()}
function dmv(e:MouseEvent){if(!drag.value)return;const a=e.clientX-dx,b=e.clientY-dy;if(Math.abs(a)>3||Math.abs(b)>3)dm=true;if(dm){x.value=Math.max(30,Math.min(innerWidth-30,px+a));y.value=Math.max(30,Math.min(innerHeight-30,py+b));tx=x.value;ty=y.value}}
function de(){drag.value=false;mood.value='idle';it=0;if(!dm)hc()}

// ── Chat ──
const cb=ref(''),cv=ref(false),cl=ref(false),ci=ref(''),cm=ref(false)
let ct:any=null,at:any=null
const im=['好无聊呀~你在写什么代码呢？','该喝杯水了哦~☕','今天修了几个bug呀？💪','你代码写得真好看！','我可以踩一下键盘吗？喵~','累了就摸摸我充电吧~','要不要休息5分钟？','我看到你写的代码了，真厉害！','今天午饭吃什么呀？',`我叫${petName.value}，请多关照！`]
function sb(t:string,d=4000){cb.value=t;cv.value=true;if(ct)clearTimeout(ct);ct=setTimeout(()=>{cv.value=false},d)}
async function hc(){
  if(mood.value==='sleep'){mood.value='idle';it=0;sb('呼...睡醒了！☀️');return}
  if(cm.value){mood.value='jump';st=0;setTimeout(()=>mood.value='idle',600);return}
  mood.value='jump';st=0;setTimeout(()=>mood.value='idle',600);sb(rnd(im))
}
async function dbc(){
  cm.value=!cm.value
  if(cm.value){mood.value='chat';sb(`嗨！我是${petName.value}（${b.value.breed}）~可以让我帮你生成日报哦 📊`,5000)}
  else{mood.value='idle';sb('那我先去玩啦~拜拜👋',2500)}
}
async function sc(){
  const m=ci.value.trim();if(!m)return;ci.value='';cl.value=true;sb('让我想想...🤔',99999)
  if(m.includes('日报')||m.includes('每日总结')||m.includes('今天做了什么')){
    const td=new Date().toISOString().slice(0,10)
    try{await generateDailyReport(td);sb('日报已开始生成！去日报页面看吧~📊',4000);setTimeout(async()=>{try{const s=await fetchGenerationStatus();const d=s.data as any;if(d?.phase==='done'||d?.from_cache){const r=await fetchDailyReport(td);const c=r.data as any;if(c?.content) sb(String(c.content).replace(/\n/g,' ').slice(0,120)+'...去日报页面看完整版~',8000)}}catch{}},3000)}catch{sb('生成失败...检查AI模型配置？📊',4000)}
  }else{
    try{const r=await api<ApiResponse<{reply:string}>>('POST','/admin/pet/chat',null,{message:m,pet_type:species.value,pet_name:petName.value});if(r.success&&r.data)sb(r.data.reply,6000);else sb(rnd(im),4000)}catch{sb('AI好像睡着了...💤',4000)}finally{cl.value=false}
  }
}

// ── Animation ──
function anim(t:number){const d=Math.min((t-lt)/1000,.1);lt=t;fl.value=Math.sin(t*.003)*4;st+=d*1000;it+=d*1000
  if(drag.value){af=requestAnimationFrame(anim);return}
  if(mood.value==='sleep')sq.value=1+Math.sin(t*.002)*.03
  else if(mood.value==='jump'){const p=(st%600)/600;sq.value=1-Math.sin(p*Math.PI)*.35;if(st>600){mood.value='idle';st=0;sq.value=1}}
  else if(mood.value==='walk'){
    const a=tx-x.value,b=ty-y.value,di=Math.sqrt(a*a+b*b)
    if(di<5){mood.value='idle';st=0}
    else{const s=45+Math.sin(t*.005)*12;vx+=(a/di*s-vx)*3*d;vy+=(b/di*s-vy)*3*d;x.value+=vx*d;y.value+=vy*d;if(Math.abs(vx)>3)facing.value=vx>0?'r':'l';fs+=d*8;sq.value=1+Math.abs(Math.sin(fs))*.05}
  }else if(mood.value==='idle'||mood.value==='chat'){sq.value=1+Math.sin(t*.004)*.025;eo.value={x:Math.sin(t*.002)*2,y:Math.cos(t*.003)*1.5};if(it>4000+Math.random()*6000){if(Math.random()<.3){mood.value='jump';st=0}else pt()}}
  if(x.value<40||x.value>innerWidth-40||y.value<40||y.value>innerHeight-40)pt()
  af=requestAnimationFrame(anim)}
onMounted(()=>{lt=performance.now();af=requestAnimationFrame(anim);setTimeout(()=>pt(),2000);addEventListener('mousemove',dmv);addEventListener('mouseup',de)
  at=setInterval(()=>{if(!cv.value&&!drag.value&&Math.random()<.3)sb(rnd(im),4000)},45000+Math.random()*60000)})
onUnmounted(()=>{cancelAnimationFrame(af);removeEventListener('mousemove',dmv);removeEventListener('mouseup',de);if(ct)clearTimeout(ct);if(at)clearInterval(at)})
</script>

<template>
<div class="sp" :class="[`s-${species}`,`f-${facing}`,`m-${mood}`]"
  :style="{left:x+'px',top:y+'px',transform:`translate(-50%,-50%) scaleX(${facing==='l'?-1:1}) scale(${sq}) translateY(${fl}px)`}"
  @mousedown="ds" @dblclick.stop="dbc">

  <!-- Ground shadow -->
  <div class="shd"></div>

  <!-- BODY -->
  <div class="bd" :style="{'--c':b.color,'--bl':b.belly,'--dk':b.dark,'--ec':b.eyeColor}">
    <!-- Belly lighter patch -->
    <div class="belly"></div>
    <!-- Back highlight stripe -->
    <div class="back-hl"></div>

    <!-- TAIL -->
    <div class="tail"><div class="tail-tip"></div></div>

    <!-- LEGS (back legs) -->
    <div class="legs-back">
      <div class="leg lb"><div class="paw"></div></div>
      <div class="leg lb"><div class="paw"></div></div>
    </div>

    <!-- HEAD -->
    <div class="hd">
      <!-- EARS -->
      <div class="ear ear-l"><div class="ear-in"></div></div>
      <div class="ear ear-r"><div class="ear-in"></div></div>

      <!-- Face markings -->
      <div class="face-mask" v-if="species==='cat'||species==='fox'||species==='panda'"></div>
      <!-- Panda eye patches -->
      <template v-if="species==='panda'">
        <div class="panda-patch patch-l"></div>
        <div class="panda-patch patch-r"></div>
      </template>

      <!-- FACE -->
      <div class="fc">
        <!-- Snout/muzzle (dogs, pandas) -->
        <div class="snout" v-if="species==='dog'||species==='panda'"></div>

        <!-- Whisker pads (cats) -->
        <div class="wsk-pads" v-if="species==='cat'"><div class="wsp"></div><div class="wsp"></div></div>

        <!-- Eyes -->
        <div class="eyes" :style="{transform:`translate(${eo.x}px,${eo.y}px)`}">
          <div class="eye"><div class="iris"><div class="pup"></div></div><div class="cl"></div></div>
          <div class="eye"><div class="iris"><div class="pup"></div></div><div class="cl"></div></div>
        </div>

        <!-- Nose -->
        <div class="nose"></div>

        <!-- Mouth -->
        <div class="mouth" v-if="mood!=='sleep'"></div>

        <!-- Whiskers -->
        <div class="wsk" v-if="species==='cat'||species==='fox'">
          <div class="w w1"></div><div class="w w2"></div><div class="w w3"></div>
          <div class="w w4"></div><div class="w w5"></div><div class="w w6"></div>
        </div>
      </div>
    </div>

    <!-- LEGS (front legs) -->
    <div class="legs-front">
      <div class="leg lf"><div class="paw"></div></div>
      <div class="leg lf"><div class="paw"></div></div>
    </div>
  </div>

  <!-- Zzz -->
  <div class="zz" v-if="mood==='sleep'"><span v-for="i in 3":key="i":style="{animationDelay:i*.4+'s'}">Z</span></div>

  <!-- Breed label -->
  <div class="blbl">{{ b.breed }}</div>

  <!-- Speech bubble -->
  <Transition name="bb"><div class="bb" v-if="cv" @mousedown.stop :style="facing==='l'?{transform:'translateX(-50%) scaleX(-1)'}:{}">{{ cb }}<div class="ba"></div></div></Transition>

  <!-- Chat bar -->
  <div class="chb" v-if="cm" @mousedown.stop @click.stop :style="facing==='l'?{transform:'translateX(-50%) scaleX(-1)'}:{}">
    <input v-model="ci" :placeholder="'和'+petName+'说点什么...'" @keyup.enter="sc" :disabled="cl" class="chi"/>
    <button class="chs" @click="sc":disabled="cl">发</button>
  </div>
</div>
</template>

<style scoped>
/* ── Container ── */
.sp{position:fixed;z-index:99999;width:0;height:0;cursor:grab;user-select:none}
.sp:active{cursor:grabbing}.m-sleep{cursor:default}

/* ── Shadow ── */
.shd{position:absolute;bottom:-28px;left:50%;transform:translateX(-50%);width:50px;height:10px;background:radial-gradient(ellipse,rgba(0,0,0,.15) 0%,transparent 70%);border-radius:50%;transition:transform .3s}
.m-jump .shd{transform:translateX(-50%) scale(.6)}

/* ── BODY (3D gradient: light from top-left) ── */
.bd{position:absolute;width:64px;height:42px;left:-32px;top:-40px;background:linear-gradient(145deg,color-mix(in srgb,var(--c) 70%,white 30%) 0%,var(--c) 40%,color-mix(in srgb,var(--c) 70%,black 30%) 100%);border-radius:45% 55% 50% 50%/55% 55% 45% 45%;box-shadow:0 5px 18px rgba(0,0,0,.18),inset 0 -6px 12px rgba(0,0,0,.08),inset 0 4px 10px rgba(255,255,255,.2)}
.belly{position:absolute;bottom:4px;left:25%;width:50%;height:40%;background:radial-gradient(ellipse,var(--bl) 0%,transparent 70%);border-radius:50%;opacity:.7}
.back-hl{position:absolute;top:4px;left:30%;width:35%;height:22%;background:radial-gradient(ellipse,rgba(255,255,255,.22) 0%,transparent 70%);border-radius:50%}

/* Species body */
.s-cat .bd{border-radius:48% 52% 50% 50%/58% 55% 42% 45%}
.s-dog .bd{width:70px;height:48px;left:-35px;top:-44px;border-radius:44% 56% 50% 50%/52% 52% 48% 48%}
.s-rabbit .bd{width:52px;height:50px;left:-26px;top:-46px;border-radius:40% 60% 50% 50%/45% 48% 52% 55%}
.s-fox .bd{width:58px;height:46px;left:-29px;top:-42px;border-radius:44% 56% 50% 50%/54% 52% 48% 46%}
.s-bird .bd{width:38px;height:44px;left:-19px;top:-36px;border-radius:55% 45% 55% 45%/56% 55% 45% 44%}
.s-panda .bd{width:68px;height:52px;left:-34px;top:-48px;border-radius:44% 50% 52% 48%/55% 54% 46% 45%}

/* ── TAIL ── */
.tail{position:absolute;bottom:8px;left:-16px;width:24px;height:8px;background:linear-gradient(90deg,var(--c),color-mix(in srgb,var(--c) 80%,black 20%));border-radius:0 65% 65% 0;transform-origin:2px center;animation:tw 1.5s ease-in-out infinite;z-index:-1}
.tail-tip{position:absolute;right:-2px;top:0;width:10px;height:8px;background:var(--dk);border-radius:0 60% 60% 0}
.s-cat .tail{width:28px;height:7px;left:-18px}
.s-dog .tail{width:18px;height:10px;left:-12px;animation-duration:.5s;border-radius:0 50% 50% 0}
.s-fox .tail{width:36px;height:10px;left:-22px;border-radius:0 75% 75% 0}
.s-rabbit .tail,.s-panda .tail{width:12px;height:12px;border-radius:50%;left:-8px;animation:none}
.s-bird .tail{width:14px;height:4px;left:-10px;bottom:18px;background:rgba(0,0,0,.12);border-radius:2px;transform-origin:left center}
.s-bird .tail-tip{display:none}
.m-sleep .tail{animation-play-state:paused}
@keyframes tw{0%,100%{transform:rotate(0)}25%{transform:rotate(18deg)}75%{transform:rotate(-12deg)}}

/* ── BACK LEGS ── */
.legs-back{position:absolute;bottom:4px;left:50%;transform:translateX(-50%);display:flex;gap:24px;z-index:-1}
.leg{width:9px;height:12px;background:linear-gradient(180deg,var(--c),color-mix(in srgb,var(--c) 80%,black 20%));border-radius:40% 40% 30% 30%}
.lb{height:10px;opacity:.7}
.paw{width:11px;height:6px;background:var(--dk);border-radius:45% 45% 40% 40%;margin-left:-1px;margin-top:-1px}

/* ── HEAD ── */
.hd{position:absolute;top:-2px;left:50%;transform:translateX(-50%);width:48px;height:42px}
.s-dog .hd{width:52px;height:46px}
.s-panda .hd{width:54px;height:48px}

/* ── EARS ── */
.ear{position:absolute;border-left:8px solid transparent;border-right:8px solid transparent;border-bottom:18px solid color-mix(in srgb,var(--c) 80%,black 20%);filter:drop-shadow(0 1px 2px rgba(0,0,0,.12))}
.ear-in{position:absolute;top:3px;left:50%;transform:translateX(-50%);width:0;height:0;border-left:4px solid transparent;border-right:4px solid transparent;border-bottom:8px solid rgba(255,200,180,.35)}.ear-l{top:-14px;left:2px;transform:rotate(-20deg)}.ear-r{top:-14px;right:2px;transform:rotate(20deg)}
.s-dog .ear{width:10px;height:18px;border:none;background:color-mix(in srgb,var(--c) 75%,black 25%);border-radius:50% 30% 5% 5%}
.s-dog .ear-l{left:0;top:-14px;border-radius:60% 30% 5% 5%}
.s-dog .ear-r{right:0;top:-14px;border-radius:30% 60% 5% 5%}
.s-dog .ear-in{display:none}
.s-rabbit .ear{border-bottom-width:28px;border-left-width:5px;border-right-width:5px}.s-rabbit .ear-in{border-bottom-width:11px;top:5px}
.s-rabbit .ear-l{left:0;top:-22px}.s-rabbit .ear-r{right:0;top:-22px}
.s-fox .ear{border-bottom-width:22px}.s-fox .ear-l{transform:rotate(-28deg)}.s-fox .ear-r{transform:rotate(28deg)}
.s-bird .ear{display:none}
.s-panda .ear{border-bottom-width:16px;border-left-width:10px;border-right-width:10px;border-radius:50%}.s-panda .ear-in{border-bottom-width:6px;top:2px}

/* ── FACE ── */
.fc{position:absolute;top:6px;left:50%;transform:translateX(-50%)}

/* Face mask (cat lighter muzzle area) */
.face-mask{position:absolute;top:8px;left:50%;transform:translateX(-50%);width:28px;height:18px;background:radial-gradient(ellipse,rgba(255,255,255,.18),transparent);border-radius:50%;pointer-events:none}

/* Snout (dog/panda) */
.snout{position:absolute;top:12px;left:50%;transform:translateX(-50%);width:18px;height:12px;background:var(--bl);border-radius:50%;opacity:.8}
.s-panda .snout{width:20px;height:14px}

/* Panda eye patches */
.panda-patch{position:absolute;top:6px;width:16px;height:14px;background:var(--dk);border-radius:45%}.patch-l{left:4px;transform:rotate(-8deg)}.patch-r{right:4px;transform:rotate(8deg)}

/* ── EYES ── */
.eyes{display:flex;gap:18px;justify-content:center;margin-bottom:2px;position:relative;z-index:1}
.s-cat .eyes,.s-fox .eyes{gap:16px}.s-dog .eyes{gap:20px}.s-bird .eyes{gap:10px}
.eye{width:11px;height:12px;background:#fff;border-radius:50% 50% 50% 50%/55% 55% 45% 45%;display:flex;align-items:center;justify-content:center;box-shadow:inset 0 2px 3px rgba(0,0,0,.08);position:relative;overflow:hidden}
.iris{width:8px;height:8px;background:var(--ec);border-radius:50%;display:flex;align-items:center;justify-content:center}.pup{width:4px;height:4px;background:#0f172a;border-radius:50%}
.cl{position:absolute;top:2px;left:2px;width:3px;height:3px;background:#fff;border-radius:50%}
.s-cat .eye{width:12px;height:13px;border-radius:50% 50% 45% 45%/62% 60% 38% 40%}
.s-dog .eye{width:10px;height:10px}
.s-bird .eye{width:7px;height:8px}.s-bird .iris{width:6px;height:6px}.s-bird .pup{width:3px;height:3px}
.s-panda .eye{width:13px;height:14px}

/* ── NOSE ── */
.nose{width:6px;height:5px;background:linear-gradient(180deg,rgba(0,0,0,.35),rgba(0,0,0,.55));border-radius:45% 45% 50% 50%;margin:2px auto 0;position:relative;z-index:1}
.nose::after{content:'';position:absolute;top:1px;left:1px;width:2px;height:1.5px;background:rgba(255,255,255,.3);border-radius:50%}
.s-cat .nose{width:5px;height:4px}
.s-dog .nose{width:10px;height:7px;background:linear-gradient(180deg,#1e293b,#0f172a)}
.s-rabbit .nose{width:4px;height:4px;background:rgba(220,160,160,.7);border-radius:50%}
.s-fox .nose{width:5px;height:4px}
.s-bird .nose{width:7px;height:4px;background:#f97316;border-radius:3px 3px 0 0}
.s-panda .nose{width:8px;height:6px}

/* Whisker pads */
.wsk-pads{display:flex;gap:12px;justify-content:center;margin-top:0;position:relative;z-index:0}.wsp{width:8px;height:6px;background:rgba(255,255,255,.25);border-radius:50%}

/* ── MOUTH ── */
.mouth{width:8px;height:4px;border-bottom:2px solid rgba(0,0,0,.18);border-radius:0 0 50% 50%;margin:2px auto 0}.m-jump .mouth{width:12px;height:6px;border-width:3px;border-radius:0 0 60% 60%}.s-dog .mouth{width:14px}

/* ── WHISKERS ── */
.wsk{position:absolute;top:16px;left:50%;transform:translateX(-50%);width:52px;pointer-events:none}
.w{position:absolute;width:20px;height:1.2px;background:rgba(0,0,0,.1);border-radius:1px}.w1{left:-14px;top:0;transform:rotate(-16deg)}.w2{left:-14px;top:3px;transform:rotate(-4deg)}.w3{left:-14px;top:6px;transform:rotate(6deg)}.w4{right:-14px;top:0;transform:rotate(16deg)}.w5{right:-14px;top:3px;transform:rotate(4deg)}.w6{right:-14px;top:6px;transform:rotate(-6deg)}

/* ── FRONT LEGS ── */
.legs-front{position:absolute;bottom:2px;left:50%;transform:translateX(-50%);display:flex;gap:20px}
.lf{height:14px}.m-walk .lf:nth-child(1){animation:ft .35s ease-in-out .18s infinite}.m-walk .lf:nth-child(2){animation:ft .35s ease-in-out infinite}.m-jump .leg{transform:translateY(-4px) scale(.7)}
@keyframes ft{0%,100%{transform:translateY(0)}50%{transform:translateY(-3px)}}

/* ── Zzz ── */
.zz{position:absolute;top:-28px;right:-14px;display:flex;flex-direction:column;font-size:11px;font-weight:700;color:#94a3b8;pointer-events:none}.zz span{animation:z 1.2s ease-out infinite;opacity:0}@keyframes z{0%{opacity:0;transform:translate(0,0)}30%{opacity:1}100%{opacity:0;transform:translate(6px,-14px)}}

/* ── Breed label ── */
.blbl{position:absolute;top:-24px;left:50%;transform:translateX(-50%);font-size:10px;color:var(--c);font-weight:600;pointer-events:none;opacity:0;transition:opacity .3s;white-space:nowrap;text-shadow:0 1px 2px rgba(255,255,255,.8)}.sp:hover .blbl{opacity:1}

/* ── Speech bubble ── */
.bb{position:absolute;bottom:56px;left:50%;transform:translateX(-50%);max-width:240px;min-width:60px;padding:10px 14px;background:#fff;color:#1e293b;border-radius:14px;font-size:13px;line-height:1.5;box-shadow:0 4px 20px rgba(0,0,0,.14);text-align:center;word-break:break-word;white-space:pre-wrap}
.ba{position:absolute;bottom:-6px;left:50%;transform:translateX(-50%);width:0;height:0;border-left:7px solid transparent;border-right:7px solid transparent;border-top:7px solid #fff}
.bb-enter-active{transition:all .25s ease}.bb-leave-active{transition:all .2s ease}.bb-enter-from,.bb-leave-to{opacity:0;transform:translateX(-50%) translateY(6px)}

/* ── Chat bar ── */
.chb{position:absolute;bottom:-50px;left:50%;transform:translateX(-50%);display:flex;gap:4px;background:#fff;padding:5px 5px;border-radius:12px;box-shadow:0 2px 12px rgba(0,0,0,.12);white-space:nowrap;z-index:2}
.chi{border:none;outline:none;padding:4px 10px;font-size:12px;width:170px;background:transparent;color:#1e293b}.chi::placeholder{color:#94a3b8}
.chs{padding:4px 12px;border:none;border-radius:7px;background:linear-gradient(135deg,#0ea5e9,#0284c7);color:#fff;font-size:12px;font-weight:600;cursor:pointer}.chs:disabled{opacity:.5}

/* ── Breathing ── */
.m-sleep .bd{animation:br 2.5s ease-in-out infinite}@keyframes br{0%,100%{transform:scale(1)}50%{transform:scale(1.04)}}
</style>
