import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { api } from '../utils/api';
import MetricTile from '../components/MetricTile';
import StatusDot from '../components/StatusDot';

export default function GridComputePanel() {
  const [status, setStatus] = useState(null);
  const [workerStatus, setWorkerStatus] = useState(null);
  const [latestBatch, setLatestBatch] = useState(null);
  const [loading, setLoading] = useState(true);
  const [workerLoading, setWorkerLoading] = useState(false);

  const fetchData = useCallback(async () => {
    try {
      const [statusData, workerData] = await Promise.all([
        api('/intelligence/status', { silent: true }),
        invoke('research_worker_status').catch(() => null),
      ]);

      if (statusData) setStatus(statusData);
      if (workerData) setWorkerStatus(workerData);

      // Fetch latest batch detail if we have a batch ID
      const batchId = statusData?.latest_batch_id
        || statusData?.last_batch_id
        || statusData?.recent_batches?.[0]?.id;

      if (batchId) {
        const batchData = await api(`/intelligence/batch/${batchId}`, { silent: true });
        if (batchData) setLatestBatch(batchData);
      }
    } catch (e) {
      console.warn('[GridCompute] fetch error:', e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    const iv = setInterval(fetchData, 10000);
    return () => clearInterval(iv);
  }, [fetchData]);

  // Auto-start worker if not running on panel mount
  useEffect(() => {
    let cancelled = false;
    const autoStart = async () => {
      try {
        const ws = await invoke('research_worker_status').catch(() => null);
        if (!cancelled && ws && !ws.running) {
          await invoke('start_research_worker').catch(() => {});
          const updated = await invoke('research_worker_status').catch(() => null);
          if (updated) setWorkerStatus(updated);
        }
      } catch { /* silent */ }
    };
    // Delay 3s to let app fully initialize
    const timer = setTimeout(autoStart, 3000);
    return () => { cancelled = true; clearTimeout(timer); };
  }, []);

  const toggleWorker = useCallback(async () => {
    setWorkerLoading(true);
    try {
      const cmd = workerStatus?.running ? 'stop_research_worker' : 'start_research_worker';
      const result = await invoke(cmd);
      setWorkerStatus(result);
    } catch (e) {
      console.warn('[GridCompute] worker toggle error:', e);
    } finally {
      setWorkerLoading(false);
    }
  }, [workerStatus]);

  if (loading && !status) {
    return <div className="p-4 text-aura-muted animate-pulse">Loading grid compute...</div>;
  }

  const queueDepth = status?.queue_depth ?? status?.pending_jobs ?? status?.queue_size
    ?? (status?.jobs_by_status ? Object.values(status.jobs_by_status).reduce((a, b) => a + b, 0) : '--');
  const workerList = status?.workers || [];
  const activeWorkers = status?.active_workers ?? status?.worker_count
    ?? workerList.filter(w => w.status === 'online' || w.status === 'busy').length
    || status?.workers_by_status ? Object.entries(status?.workers_by_status || {}).filter(([s]) => s !== 'dead' && s !== 'offline').reduce((a, [, v]) => a + v, 0) : '--';
  const completed = status?.jobs_completed ?? status?.completed ?? status?.total_completed
    ?? status?.jobs_by_status?.completed ?? '--';
  const throughputRaw = status?.throughput ?? status?.jobs_per_min ?? status?.jobs_per_minute
    ?? (status?.throughput_1h != null ? (status.throughput_1h / 60) : null);
  const throughput = throughputRaw != null ? throughputRaw : '--';
  const recentBatches = status?.recent_batches || status?.batches || [];
  const isWorkerRunning = workerStatus?.running || false;

  // Per-device throughput — max for bar scaling
  const maxJobsPerMin = workerList.length > 0
    ? Math.max(...workerList.map(w => w.jobs_per_min || 0), 0.01)
    : 0;

  // Latest batch details
  const batchResults = latestBatch?.results || latestBatch?.jobs || latestBatch?.items || [];
  const batchStatus = latestBatch?.status || latestBatch?.state || '';
  const batchProgress = latestBatch?.progress ?? latestBatch?.completed_pct;

  return (
    <div className="space-y-3 overflow-auto">
      {/* Grid metrics */}
      <div className="grid grid-cols-4 gap-2">
        <MetricTile label="Queue" value={queueDepth} size="sm" />
        <MetricTile label="Workers" value={activeWorkers} size="sm" />
        <MetricTile
          label="Completed"
          value={typeof completed === 'number' ? completed.toLocaleString() : completed}
          size="sm"
        />
        <MetricTile
          label="Jobs/min"
          value={typeof throughput === 'number' ? throughput.toFixed(1) : throughput}
          size="sm"
        />
      </div>

      {/* Per-device throughput bars */}
      {workerList.length > 0 && (
        <div className="glass-panel p-3">
          <div className="text-xs font-medium text-aura-muted mb-2">Jobs/min — per device</div>
          <div className="space-y-1.5">
            {workerList.map((w) => {
              const pct = maxJobsPerMin > 0 ? Math.min((w.jobs_per_min / maxJobsPerMin) * 100, 100) : 0;
              const isOnline = w.status === 'online' || w.status === 'busy';
              return (
                <div key={w.id}>
                  <div className="flex items-center justify-between text-xs mb-0.5">
                    <span className={`truncate max-w-[140px] ${isOnline ? 'text-aura-text' : 'text-aura-muted'}`}>
                      {w.hostname || w.id}
                    </span>
                    <span className="font-mono text-aura-amber ml-2 shrink-0">
                      {w.jobs_per_min.toFixed(2)}/min
                    </span>
                  </div>
                  <div className="w-full h-1.5 rounded-full bg-aura-border overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all"
                      style={{
                        width: `${pct}%`,
                        background: isOnline ? '#D29922' : '#30363D',
                      }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Worker control */}
      <div className="glass-panel p-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <StatusDot color={isWorkerRunning ? 'green' : 'red'} pulse={isWorkerRunning} />
            <div>
              <div className="text-xs font-medium text-aura-text">
                Desktop Worker {isWorkerRunning ? 'Running' : 'Stopped'}
              </div>
              {workerStatus?.pid && (
                <div className="text-xs text-aura-muted font-mono">PID: {workerStatus.pid}</div>
              )}
            </div>
          </div>
          <button
            onClick={toggleWorker}
            disabled={workerLoading}
            className={`text-xs px-3 py-1.5 rounded font-medium transition-colors ${
              isWorkerRunning
                ? 'bg-aura-red/15 text-aura-red hover:bg-aura-red/25 border border-aura-red/30'
                : 'bg-aura-green/15 text-aura-green hover:bg-aura-green/25 border border-aura-green/30'
            } disabled:opacity-50`}
          >
            {workerLoading ? '...' : isWorkerRunning ? 'Stop' : 'Start'}
          </button>
        </div>
        {workerStatus?.project_path && (
          <div className="text-xs text-aura-muted mt-1 truncate" title={workerStatus.project_path}>
            {workerStatus.project_path}
          </div>
        )}
      </div>

      {/* Latest batch detail */}
      {latestBatch && (
        <div className="glass-panel p-3">
          <div className="flex items-center justify-between mb-2">
            <div className="text-xs font-medium text-aura-text">
              Latest Batch
              {latestBatch.id && (
                <span className="ml-1 text-aura-muted font-mono">#{latestBatch.id}</span>
              )}
            </div>
            {batchStatus && (
              <span
                className="text-xs px-1.5 py-0.5 rounded"
                style={{
                  background: batchStatus === 'completed' ? 'rgba(63,185,80,0.15)' : batchStatus === 'failed' ? 'rgba(248,81,73,0.15)' : 'rgba(210,153,34,0.15)',
                  color: batchStatus === 'completed' ? '#3FB950' : batchStatus === 'failed' ? '#F85149' : '#D29922',
                }}
              >
                {batchStatus}
              </span>
            )}
          </div>

          {batchProgress != null && (
            <div className="mb-2">
              <div className="flex items-center justify-between text-xs text-aura-muted mb-0.5">
                <span>Progress</span>
                <span className="font-mono">{typeof batchProgress === 'number' ? `${Math.round(batchProgress)}%` : batchProgress}</span>
              </div>
              <div className="w-full h-1.5 rounded-full bg-aura-border overflow-hidden">
                <div
                  className="h-full rounded-full bg-aura-blue transition-all"
                  style={{ width: `${Math.min(typeof batchProgress === 'number' ? batchProgress : 0, 100)}%` }}
                />
              </div>
            </div>
          )}

          {batchResults.length > 0 && (
            <div className="space-y-1 max-h-40 overflow-auto">
              {batchResults.slice(0, 10).map((r, i) => (
                <div key={i} className="flex items-center justify-between text-xs py-0.5 border-b border-aura-border/30 last:border-0">
                  <div className="flex items-center gap-1.5">
                    <StatusDot
                      color={r.status === 'completed' || r.status === 'success' ? 'green' : r.status === 'failed' || r.status === 'error' ? 'red' : 'amber'}
                      size="sm"
                    />
                    <span className="text-aura-text font-mono truncate max-w-[120px]">
                      {r.symbol || r.task || r.job_type || r.name || `Job ${i + 1}`}
                    </span>
                  </div>
                  <span className="text-aura-muted">
                    {r.duration ? `${r.duration}s` : r.status || ''}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Recent batches list */}
      {recentBatches.length > 0 && (
        <div className="glass-panel p-3">
          <div className="text-xs text-aura-muted mb-2 font-medium">Recent Batches</div>
          <div className="space-y-1">
            {recentBatches.slice(0, 8).map((b, i) => {
              const bId = b.id || b.batch_id || i;
              const bSt = b.status || b.state || 'unknown';
              const bJobs = b.job_count || b.total_jobs || b.count || 0;
              const bTime = b.created_at || b.started_at || b.time || '';

              return (
                <div key={bId} className="flex items-center justify-between text-xs py-0.5 border-b border-aura-border/30 last:border-0">
                  <div className="flex items-center gap-1.5">
                    <StatusDot
                      color={bSt === 'completed' ? 'green' : bSt === 'running' || bSt === 'in_progress' ? 'amber' : bSt === 'failed' ? 'red' : 'muted'}
                      size="sm"
                    />
                    <span className="font-mono text-aura-text">#{bId}</span>
                  </div>
                  <span className="text-aura-muted">{bJobs} jobs</span>
                  <span className="text-aura-muted capitalize">{bSt}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Fallback for no data */}
      {!status && !loading && (
        <div className="text-sm text-aura-muted text-center py-8">Grid compute status unavailable</div>
      )}
    </div>
  );
}
