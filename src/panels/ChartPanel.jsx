import { useState, useEffect, useRef } from 'react';
import { createChart } from 'lightweight-charts';
import useChartData from '../hooks/useChartData';

const TIMEFRAMES = ['1D', '1W', '1M', '3M', '6M', '1Y', 'ALL'];
const POPULAR = ['SPY', 'QQQ', 'AAPL', 'MSFT', 'NVDA', 'TSLA', 'AMZN', 'GOOGL', 'META', 'AMD', 'BTCUSD', 'ETHUSD'];

export default function ChartPanel() {
  const chartRef = useRef(null);
  const containerRef = useRef(null);
  const seriesRef = useRef(null);
  const volumeRef = useRef(null);
  const [symbol, setSymbol] = useState('SPY');
  const [inputVal, setInputVal] = useState('SPY');
  const [timeframe, setTimeframe] = useState('6M');
  const { fetchChart, loading, error } = useChartData();

  const loadChart = (sym, tf) => {
    fetchChart(sym, tf, {
      onCandles: (candles) => {
        if (seriesRef.current) seriesRef.current.setData(candles);
        chartRef.current?.timeScale().fitContent();
      },
      onVolumes: (volumes) => {
        if (volumeRef.current) volumeRef.current.setData(volumes);
      },
    });
  };

  const [chartError, setChartError] = useState(null);

  useEffect(() => {
    if (!containerRef.current) return;
    let chart;
    try {
      chart = createChart(containerRef.current, {
      layout: { background: { color: '#0D1117' }, textColor: '#8B949E', fontFamily: "'Plus Jakarta Sans', sans-serif", fontSize: 11 },
      grid: { vertLines: { color: '#1C2128' }, horzLines: { color: '#1C2128' } },
      crosshair: { mode: 0, vertLine: { color: '#58A6FF', width: 1, style: 2, labelBackgroundColor: '#161B22' }, horzLine: { color: '#58A6FF', width: 1, style: 2, labelBackgroundColor: '#161B22' } },
      rightPriceScale: { borderColor: '#30363D', scaleMargins: { top: 0.1, bottom: 0.25 } },
      timeScale: { borderColor: '#30363D', timeVisible: false },
      handleScale: { mouseWheel: true, pinch: true },
      handleScroll: { mouseWheel: true, pressedMouseMove: true },
    });

    const candleSeries = chart.addCandlestickSeries({
      upColor: '#3FB950', downColor: '#F85149',
      borderUpColor: '#3FB950', borderDownColor: '#F85149',
      wickUpColor: '#3FB950', wickDownColor: '#F85149',
    });

    const volumeSeries = chart.addHistogramSeries({
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    });
    chart.priceScale('volume').applyOptions({ scaleMargins: { top: 0.8, bottom: 0 } });

    chartRef.current = chart;
    seriesRef.current = candleSeries;
    volumeRef.current = volumeSeries;

    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        chart.applyOptions({ width: entry.contentRect.width, height: entry.contentRect.height });
      }
    });
    ro.observe(containerRef.current);

    loadChart(symbol, timeframe);

    return () => {
      ro.disconnect();
      chart.remove();
    };
    } catch (err) {
      console.error('Chart init failed:', err);
      setChartError(String(err));
    }
  }, []);

  useEffect(() => { loadChart(symbol, timeframe); }, [symbol, timeframe]);

  const handleSymbolSubmit = (e) => {
    e.preventDefault();
    const s = inputVal.trim().toUpperCase();
    if (s) { setSymbol(s); setInputVal(s); }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-2 p-2 border-b border-aura-border flex-shrink-0">
        <form onSubmit={handleSymbolSubmit} className="flex items-center gap-1">
          <input
            value={inputVal}
            onChange={(e) => setInputVal(e.target.value.toUpperCase())}
            className="w-24 px-2 py-1 text-xs font-mono bg-aura-surface2 border border-aura-border rounded text-aura-text outline-none focus:border-aura-blue"
            placeholder="Symbol"
          />
        </form>
        <div className="flex gap-0.5">
          {TIMEFRAMES.map(tf => (
            <button
              key={tf}
              onClick={() => setTimeframe(tf)}
              className={`text-xs px-2 py-1 rounded transition-colors ${tf === timeframe ? 'bg-aura-blue text-white' : 'text-aura-muted hover:text-aura-text hover:bg-aura-surface2'}`}
            >
              {tf}
            </button>
          ))}
        </div>
        {loading && <span className="text-xs text-aura-muted animate-pulse ml-2">Loading...</span>}
        {error && <span className="text-xs text-aura-red ml-2">{error}</span>}
      </div>
      {/* Quick symbols */}
      <div className="flex gap-1 px-2 py-1 border-b border-aura-border/50 flex-shrink-0 overflow-x-auto">
        {POPULAR.map(s => (
          <button
            key={s}
            onClick={() => { setSymbol(s); setInputVal(s); }}
            className={`text-xs px-2 py-0.5 rounded whitespace-nowrap ${s === symbol ? 'bg-aura-blue/20 text-aura-blue' : 'text-aura-muted hover:text-aura-text'}`}
          >
            {s}
          </button>
        ))}
      </div>
      {/* Chart */}
      {chartError ? (
        <div className="flex-1 flex items-center justify-center text-aura-muted text-sm">
          <div className="text-center">
            <p className="mb-1">Chart unavailable on this device</p>
            <p className="text-xs text-aura-muted/60">WebGL not supported: {chartError}</p>
          </div>
        </div>
      ) : (
        <div ref={containerRef} className="flex-1 min-h-0" />
      )}
    </div>
  );
}
