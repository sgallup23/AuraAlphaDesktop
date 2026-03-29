import{a as l,j as e}from"./vendor-C5RdOJEA.js";import{a as A}from"./index-CQ9IhrRi.js";import"./i18n-CAIaW0Ah.js";import"./motion-DriDzeSM.js";import"./radix-CM31WTz4.js";const S=()=>{const[h,d]=l.useState([]),[o,g]=l.useState("all"),[i,u]=l.useState("return30d"),[c,p]=l.useState(!0),[s,b]=l.useState({});l.useEffect(()=>{let r=!0;return A("/leaderboard").then(t=>{r&&Array.isArray(t)?d(t):r&&d([])}).catch(()=>{r&&d([])}),()=>{r=!1}},[]);const x=h.filter(r=>o==="verified"?r.verified:!0).slice().sort((r,t)=>{let n=r[i],a=t[i];return(i==="return30d"||i==="winRate")&&(n=Number(n),a=Number(a)),c?a-n:n-a}),f=r=>{b(t=>({...t,[r]:!t[r]}))},m=r=>r===1?e.jsx("span",{"aria-label":"Gold badge",title:"Gold badge",style:{color:"#F59E0B",marginRight:8,fontSize:18},children:"🥇"}):r===2?e.jsx("span",{"aria-label":"Silver badge",title:"Silver badge",style:{color:"#90A3AF",marginRight:8,fontSize:18},children:"🥈"}):r===3?e.jsx("span",{"aria-label":"Bronze badge",title:"Bronze badge",style:{color:"#EF4444",marginRight:8,fontSize:18},children:"🥉"}):null;return e.jsxs(e.Fragment,{children:[e.jsx("style",{children:`
        @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono&display=swap');
        * {
          box-sizing: border-box;
        }
        .leaderboard {
          font-family: 'JetBrains Mono', monospace;
          background-color: #0B0F14;
          color: #E5E7DB;
          padding: 20px;
          border-radius: 8px;
          max-width: 100%;
          overflow-x: auto;
          user-select: none;
        }
        .tabs {
          display: flex;
          gap: 12px;
          margin-bottom: 16px;
        }
        .tab {
          cursor: pointer;
          padding: 8px 16px;
          border-radius: 6px;
          border: 1px solid #1F2A37;
          background-color: #0F1620;
          color: #90A3AF;
          transition: background-color 0.25s, color 0.25s;
          user-select: none;
        }
        .tab.active {
          background-color: #60A5FA;
          color: #0B0F14;
          border-color: #60A5FA;
          font-weight: 600;
        }
        .sort-select {
          margin-left: auto;
          background-color: #0F1620;
          border: 1px solid #1F2A37;
          color: #E5E7DB;
          padding: 6px 12px;
          border-radius: 6px;
          font-family: 'JetBrains Mono', monospace;
          cursor: pointer;
          user-select: none;
        }
        table {
          width: 100%;
          border-collapse: collapse;
          min-width: 720px;
        }
        thead {
          background-color: #0F1620;
        }
        th, td {
          padding: 12px 10px;
          border-bottom: 1px solid #1F2A37;
          text-align: left;
          vertical-align: middle;
          white-space: nowrap;
        }
        th {
          font-weight: 600;
          font-size: 14px;
          color: #90A3AF;
          user-select: none;
        }
        tbody tr:hover {
          background-color: #16202B;
        }
        .verified-badge {
          color: #60A5FA;
          margin-left: 6px;
          vertical-align: middle;
          font-size: 14px;
        }
        .strategy-tag {
          background-color: #1F2A37;
          color: #90A3AF;
          padding: 2px 8px;
          border-radius: 12px;
          font-size: 12px;
          user-select: none;
        }
        .return-positive {
          color: #22C55E;
          font-weight: 600;
        }
        .return-negative {
          color: #EF4444;
          font-weight: 600;
        }
        .follow-button {
          background-color: transparent;
          border: 1px solid #60A5FA;
          color: #60A5FA;
          padding: 6px 14px;
          border-radius: 6px;
          font-family: 'JetBrains Mono', monospace;
          font-weight: 600;
          cursor: pointer;
          user-select: none;
          transition: background-color 0.25s, color 0.25s;
        }
        .follow-button.following {
          background-color: #60A5FA;
          color: #0B0F14;
          border-color: #60A5FA;
        }
        .follow-button:hover {
          background-color: #60A5FA;
          color: #0B0F14;
          border-color: #60A5FA;
        }
        .sort-container {
          display: flex;
          align-items: center;
          margin-bottom: 12px;
          gap: 12px;
          flex-wrap: wrap;
        }
        .sort-label {
          color: #90A3AF;
          font-size: 14px;
          user-select: none;
        }
        .sort-direction {
          cursor: pointer;
          user-select: none;
          color: #60A5FA;
          font-weight: 700;
          font-size: 18px;
          line-height: 1;
          display: inline-flex;
          align-items: center;
          justify-content: center;
          width: 20px;
          height: 20px;
          border-radius: 4px;
          border: 1px solid #60A5FA;
          transition: background-color 0.25s, color 0.25s;
        }
        .sort-direction:hover {
          background-color: #60A5FA;
          color: #0B0F14;
        }
      `}),e.jsxs("section",{className:"leaderboard","aria-label":"Aura Alpha Social Trading Leaderboard",children:[e.jsxs("div",{className:"tabs",role:"tablist","aria-label":"Filter traders",children:[e.jsx("button",{role:"tab","aria-selected":o==="all",tabIndex:o==="all"?0:-1,className:`tab${o==="all"?" active":""}`,onClick:()=>g("all"),type:"button",children:"All"}),e.jsx("button",{role:"tab","aria-selected":o==="verified",tabIndex:o==="verified"?0:-1,className:`tab${o==="verified"?" active":""}`,onClick:()=>g("verified"),type:"button",children:"Verified Only"}),e.jsxs("div",{className:"sort-container",style:{marginLeft:"auto"},children:[e.jsx("label",{htmlFor:"sort-select",className:"sort-label",children:"Sort by:"}),e.jsxs("select",{id:"sort-select",className:"sort-select",value:i,onChange:r=>u(r.target.value),"aria-label":"Sort traders by",children:[e.jsx("option",{value:"return30d",children:"Return (30d)"}),e.jsx("option",{value:"winRate",children:"Win Rate"}),e.jsx("option",{value:"followers",children:"Followers"})]}),e.jsx("button",{type:"button","aria-label":c?"Sort descending":"Sort ascending",className:"sort-direction",onClick:()=>p(r=>!r),children:c?"↓":"↑"})]})]}),e.jsxs("table",{role:"table","aria-describedby":"leaderboard-desc",children:[e.jsx("caption",{id:"leaderboard-desc",style:{position:"absolute",left:"-9999px",height:1,width:1,overflow:"hidden"},children:"Social trading leaderboard showing top traders with rank, username, verified badge, 30 day return percentage, win rate, total trades, follower count, strategy style tag, and follow button."}),e.jsx("thead",{children:e.jsxs("tr",{children:[e.jsx("th",{scope:"col",style:{width:60,textAlign:"center"},children:"#"}),e.jsx("th",{scope:"col",style:{minWidth:140},children:"Trader"}),e.jsx("th",{scope:"col",style:{width:110,textAlign:"right"},children:"30d Return"}),e.jsx("th",{scope:"col",style:{width:90,textAlign:"right"},children:"Win Rate"}),e.jsx("th",{scope:"col",style:{width:90,textAlign:"right"},children:"Trades"}),e.jsx("th",{scope:"col",style:{width:110,textAlign:"right"},children:"Followers"}),e.jsx("th",{scope:"col",style:{minWidth:110},children:"Strategy"}),e.jsx("th",{scope:"col",style:{width:110,textAlign:"center"},children:"Follow"})]})}),e.jsxs("tbody",{children:[x.map(r=>{const t=Number(r.return30d)>=0;return e.jsxs("tr",{children:[e.jsxs("td",{style:{textAlign:"center",fontWeight:"600"},children:[m(r.rank),r.rank]}),e.jsxs("td",{children:[e.jsx("span",{children:r.username}),r.verified&&e.jsx("svg",{className:"verified-badge","aria-label":"Verified trader",role:"img",viewBox:"0 0 24 24",fill:"none",stroke:"currentColor",strokeWidth:"3",strokeLinecap:"round",strokeLinejoin:"round",width:"16",height:"16",style:{verticalAlign:"middle"},children:e.jsx("polyline",{points:"20 6 9 17 4 12"})})]}),e.jsxs("td",{style:{textAlign:"right"},className:t?"return-positive":"return-negative",children:[(Number(r.return30d)*100).toFixed(2),"%"]}),e.jsxs("td",{style:{textAlign:"right"},children:[(Number(r.winRate)*100).toFixed(2),"%"]}),e.jsx("td",{style:{textAlign:"right"},children:r.totalTrades}),e.jsx("td",{style:{textAlign:"right"},children:r.followers.toLocaleString()}),e.jsx("td",{children:e.jsx("span",{className:"strategy-tag","aria-label":`Strategy style: ${r.strategy}`,children:r.strategy})}),e.jsx("td",{style:{textAlign:"center"},children:e.jsx("button",{type:"button",className:`follow-button${s[r.username]?" following":""}`,onClick:()=>f(r.username),"aria-pressed":s[r.username]?"true":"false","aria-label":`${s[r.username]?"Unfollow":"Follow"} ${r.username}`,children:s[r.username]?"Following":"Follow"})})]},r.username)}),x.length===0&&e.jsx("tr",{children:e.jsx("td",{colSpan:"8",style:{textAlign:"center",padding:"24px 0",color:"#90A3AF"},children:"No traders found."})})]})]})]})]})};export{S as default};
//# sourceMappingURL=VerifiedLeaderboard-G87688Kj.js.map
