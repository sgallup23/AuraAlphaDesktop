import{a,j as t}from"./vendor-C5RdOJEA.js";import{a as y}from"./index-CQ9IhrRi.js";import"./i18n-CAIaW0Ah.js";import"./motion-DriDzeSM.js";import"./radix-CM31WTz4.js";const e={bg:e.bg,surface:e.surface,border:"#1F2A37",text:"#E5E7DB",muted:"#90A3AF",green:e.green,red:e.red,blue:e.blue},k={bullish:e.green,bearish:e.red,neutral:e.muted},c=[{title:"AI Revolutionizes Market Analysis",source:"Aura Alpha",sentiment:"bullish",summary:"Artificial intelligence is transforming how investors analyze stocks. This new approach promises faster and more accurate insights.",timestamp:new Date(Date.now()-3600*1e3*2).toISOString()},{title:"Concerns Over Market Volatility",source:"Aura Alpha",sentiment:"bearish",summary:"Recent fluctuations have investors worried about potential downturns. Experts advise caution in the coming weeks.",timestamp:new Date(Date.now()-3600*1e3*5).toISOString()},{title:"Steady Growth Expected Amid Uncertainty",source:"Aura Alpha",sentiment:"neutral",summary:"Despite mixed signals, the market shows signs of steady growth. Analysts remain cautiously optimistic about future trends.",timestamp:new Date(Date.now()-3600*1e3*8).toISOString()}];function j(s){return new Date(s).toLocaleString(void 0,{month:"short",day:"numeric",hour:"2-digit",minute:"2-digit"})}function D({symbol:s}){const[o,u]=a.useState(s||""),[m,i]=a.useState([]),[n,p]=a.useState(!1),[v,g]=a.useState(null);a.useEffect(()=>{s&&u(s)},[s]);const x=async()=>{if(!o){i(c);return}p(!0),g(null);try{const r=await y(`/news/${encodeURIComponent(o)}`);if(!Array.isArray(r))throw new Error("Invalid data format");i(r)}catch{i(c)}finally{p(!1)}};a.useEffect(()=>{if(!o){i(c);return}x()},[o]);const l=o||"Market";return t.jsxs(t.Fragment,{children:[t.jsx("style",{children:`
        @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono&display=swap');
        .ticker-news-digest {
          font-family: 'JetBrains Mono', monospace;
          background-color: ${e.bg};
          color: ${e.text};
          border: 1px solid ${e.border};
          border-radius: 8px;
          max-width: 720px;
          padding: 12px 16px;
          user-select: none;
        }
        .header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 12px;
        }
        .title {
          font-weight: 700;
          font-size: 1.1rem;
          color: ${e.blue};
        }
        button.refresh {
          background-color: transparent;
          border: 1px solid ${e.border};
          border-radius: 4px;
          color: ${e.muted};
          font-size: 0.85rem;
          padding: 4px 10px;
          cursor: pointer;
          transition: color 0.25s, border-color 0.25s;
        }
        button.refresh:hover {
          color: ${e.blue};
          border-color: ${e.blue};
        }
        ul.news-list {
          list-style: none;
          margin: 0;
          padding: 0;
          display: flex;
          flex-direction: column;
          gap: 10px;
        }
        li.news-item {
          background-color: ${e.surface};
          border: 1px solid ${e.border};
          border-radius: 6px;
          padding: 10px 14px;
          display: flex;
          flex-direction: column;
          gap: 4px;
          cursor: default;
          transition: background-color 0.15s;
        }
        li.news-item:hover {
          background-color: ${e.border};
        }
        .news-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          gap: 8px;
          flex-wrap: wrap;
        }
        .news-title {
          font-weight: 600;
          font-size: 0.95rem;
          color: ${e.text};
          flex: 1 1 auto;
          min-width: 0;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .sentiment-badge {
          font-weight: 700;
          font-size: 0.75rem;
          padding: 2px 8px;
          border-radius: 12px;
          color: ${e.bg};
          user-select: none;
          flex-shrink: 0;
          text-transform: capitalize;
        }
        .news-source-time {
          font-size: 0.75rem;
          color: ${e.muted};
          display: flex;
          gap: 8px;
          flex-wrap: wrap;
        }
        .news-summary {
          font-size: 0.85rem;
          color: ${e.text};
          line-height: 1.3;
          max-height: 3.4em;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .skeleton {
          background-color: ${e.border};
          border-radius: 6px;
          height: 72px;
          animation: pulse 1.5s ease-in-out infinite;
        }
        @keyframes pulse {
          0% {
            background-color: ${e.border};
          }
          50% {
            background-color: ${e.surface};
          }
          100% {
            background-color: ${e.border};
          }
        }
        .error-message {
          color: ${e.red};
          font-size: 0.85rem;
          text-align: center;
          margin-top: 8px;
        }
      `}),t.jsxs("section",{className:"ticker-news-digest","aria-label":`News digest for ${l}`,children:[t.jsxs("div",{className:"header",children:[t.jsxs("h2",{className:"title",children:[l.toUpperCase()," News"]}),t.jsxs("div",{style:{display:"flex",gap:8,alignItems:"center"},children:[!s&&t.jsx("input",{type:"text",placeholder:"Symbol...",value:o,onChange:r=>u(r.target.value.toUpperCase()),style:{background:e.surface,border:`1px solid ${e.border}`,borderRadius:6,color:e.text,fontFamily:"'JetBrains Mono', monospace",fontSize:"0.85rem",padding:"4px 8px",width:80,outline:"none"}}),t.jsx("button",{className:"refresh",onClick:x,"aria-label":`Refresh news for ${l}`,disabled:n,type:"button",children:n?"Loading...":"Refresh"})]})]}),n?t.jsx("ul",{className:"news-list","aria-busy":"true","aria-live":"polite",children:[1,2,3].map(r=>t.jsx("li",{className:"skeleton"},r))}):m.length>0?t.jsx("ul",{className:"news-list",children:m.map(({title:r,source:b,sentiment:d,summary:f,timestamp:h},w)=>t.jsxs("li",{className:"news-item",tabIndex:0,children:[t.jsxs("div",{className:"news-header",children:[t.jsx("div",{className:"news-title",title:r,children:r}),t.jsx("div",{className:"sentiment-badge",style:{backgroundColor:k[d]||e.muted},"aria-label":`Sentiment: ${d}`,children:d})]}),t.jsx("div",{className:"news-summary",title:f,children:f}),t.jsxs("div",{className:"news-source-time",children:[t.jsx("span",{children:b}),t.jsx("span",{children:"·"}),t.jsx("time",{dateTime:h,children:j(h)})]})]},w))}):t.jsx("p",{className:"error-message",role:"alert",children:"No news available."})]})]})}export{D as default};
//# sourceMappingURL=TickerNewsDigest-BTB1lWTr.js.map
