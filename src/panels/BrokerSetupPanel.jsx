import { useState } from 'react';
import useBrokerSetup from '../hooks/useBrokerSetup';

const TIER_LABEL = {
  official: 'Official API',
  official_gateway: 'Gateway Required',
  unofficial: 'Unofficial',
};

const TIER_BG = {
  official: 'bg-aura-green/10 text-aura-green',
  official_gateway: 'bg-aura-amber/10 text-aura-amber',
  unofficial: 'bg-aura-red/10 text-aura-red',
};

export default function BrokerSetupPanel() {
  const {
    brokers, configured, message, setMessage,
    saveBroker, deleteBroker, isConfigured,
  } = useBrokerSetup();

  const [expandedId, setExpandedId] = useState(null);
  const [formData, setFormData] = useState({});
  const [waiverAccepted, setWaiverAccepted] = useState({});
  const [saving, setSaving] = useState(null);
  const [deleting, setDeleting] = useState(null);

  const toggleExpand = (id) => {
    setExpandedId(prev => prev === id ? null : id);
    setMessage(null);
  };

  const handleFieldChange = (brokerId, fieldName, value) => {
    setFormData(prev => ({
      ...prev,
      [brokerId]: { ...(prev[brokerId] || {}), [fieldName]: value },
    }));
  };

  const handleSave = async (broker) => {
    setSaving(broker.id);
    try {
      const credentials = formData[broker.id] || {};
      const ok = await saveBroker(broker, credentials, waiverAccepted[broker.id] || false);
      if (!ok) {
        setSaving(null);
        return;
      }
    } catch {
      // message already set by hook
    } finally {
      setSaving(null);
    }
  };

  const handleDelete = async (broker) => {
    setDeleting(broker.id);
    try {
      await deleteBroker(broker.id);
      setFormData(prev => { const next = { ...prev }; delete next[broker.id]; return next; });
    } catch {
      // message already set by hook
    } finally {
      setDeleting(null);
    }
  };

  // Sort: configured first, then by tier
  const tierOrder = { official: 0, official_gateway: 1, unofficial: 2 };
  const sortedBrokers = [...brokers].sort((a, b) => {
    const aConf = isConfigured(a.id) ? 0 : 1;
    const bConf = isConfigured(b.id) ? 0 : 1;
    if (aConf !== bConf) return aConf - bConf;
    return (tierOrder[a.risk_tier] || 9) - (tierOrder[b.risk_tier] || 9);
  });

  return (
    <div className="space-y-2">
      {/* Summary */}
      <div className="flex items-center gap-4 text-xs text-aura-muted">
        <span><strong className="text-aura-text">{brokers.length}</strong> available</span>
        <span><strong className="text-aura-green">{configured.length}</strong> configured</span>
        <span><strong className="text-aura-muted">{brokers.length - configured.length}</strong> pending</span>
      </div>

      {/* Message */}
      {message && (
        <div className={`px-3 py-2 rounded text-xs border ${
          message.type === 'error'
            ? 'bg-aura-red/10 text-aura-red border-aura-red/20'
            : 'bg-aura-green/10 text-aura-green border-aura-green/20'
        }`}>
          {message.text}
        </div>
      )}

      {/* Broker list */}
      {sortedBrokers.length === 0 && (
        <div className="text-xs text-aura-muted text-center py-8">Loading brokers...</div>
      )}

      <div className="space-y-1.5">
        {sortedBrokers.map(broker => {
          const expanded = expandedId === broker.id;
          const conf = isConfigured(broker.id);
          const tierCls = TIER_BG[broker.risk_tier] || 'bg-aura-surface2 text-aura-muted';

          return (
            <div
              key={broker.id}
              className={`glass-panel overflow-hidden ${conf ? 'border-l-2 border-l-aura-green' : ''}`}
            >
              {/* Header */}
              <div
                className="flex items-center justify-between px-3 py-2 cursor-pointer hover:bg-aura-surface2/50"
                onClick={() => toggleExpand(broker.id)}
              >
                <div className="flex items-center gap-2 min-w-0">
                  <span className={`status-dot ${conf ? 'bg-aura-green' : 'bg-aura-border'}`} />
                  <span className="text-xs font-semibold text-aura-text truncate">{broker.name}</span>
                  <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-semibold uppercase tracking-wide ${tierCls}`}>
                    {TIER_LABEL[broker.risk_tier] || broker.risk_tier}
                  </span>
                  {conf && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full font-semibold uppercase bg-aura-green/10 text-aura-green">
                      Configured
                    </span>
                  )}
                </div>
                <span className={`text-aura-muted text-sm transition-transform ${expanded ? 'rotate-180' : ''}`}>
                  &#x25BE;
                </span>
              </div>

              {/* Description + features */}
              <div className="px-3 pb-2">
                <p className="text-xs text-aura-muted leading-relaxed">{broker.description}</p>
                <div className="flex gap-1.5 mt-1.5 flex-wrap">
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-aura-surface2 text-aura-muted border border-aura-border">
                    Commission: {broker.commission === 'free' ? 'Free' : broker.commission}
                  </span>
                  {broker.fractional && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-aura-surface2 text-aura-muted border border-aura-border">Fractional</span>
                  )}
                  {broker.paper_trading && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-aura-surface2 text-aura-muted border border-aura-border">Paper</span>
                  )}
                </div>
              </div>

              {/* Expanded credential form */}
              {expanded && (
                <div
                  className="px-3 pb-3 pt-2 border-t border-aura-border"
                  onClick={(e) => e.stopPropagation()}
                >
                  {/* Waiver */}
                  {broker.waiver_required && broker.waiver_text && (
                    <div className={`rounded-lg p-3 mb-3 border ${
                      broker.risk_tier === 'unofficial'
                        ? 'bg-aura-red/5 border-aura-red/20'
                        : 'bg-aura-amber/5 border-aura-amber/20'
                    }`}>
                      <p className={`text-[11px] leading-relaxed ${
                        broker.risk_tier === 'unofficial' ? 'text-aura-red' : 'text-aura-amber'
                      }`}>
                        {broker.waiver_text}
                      </p>
                      <label className="flex items-center gap-2 mt-2 text-xs text-aura-text cursor-pointer">
                        <input
                          type="checkbox"
                          checked={waiverAccepted[broker.id] || false}
                          onChange={(e) =>
                            setWaiverAccepted(prev => ({ ...prev, [broker.id]: e.target.checked }))
                          }
                          className="accent-aura-blue"
                        />
                        I acknowledge and accept the risks
                      </label>
                    </div>
                  )}

                  {/* Credential fields */}
                  <div className="grid grid-cols-2 gap-2 mb-2">
                    {broker.credential_fields.map(field => (
                      <div key={field.name}>
                        <label className="block text-[11px] text-aura-muted font-medium mb-1">
                          {field.label}
                          {field.required && <span className="text-aura-red ml-0.5">*</span>}
                        </label>
                        <input
                          type={field.field_type === 'password' ? 'password' : field.field_type === 'number' ? 'number' : 'text'}
                          placeholder={field.placeholder}
                          value={(formData[broker.id] || {})[field.name] || ''}
                          onChange={(e) => handleFieldChange(broker.id, field.name, e.target.value)}
                          className="input-field text-xs py-1.5"
                        />
                      </div>
                    ))}
                  </div>

                  {/* Action buttons */}
                  <div className="flex gap-2">
                    <button
                      className="btn-primary text-xs py-1.5 px-3 disabled:opacity-50"
                      disabled={saving === broker.id || (broker.waiver_required && !waiverAccepted[broker.id])}
                      onClick={() => handleSave(broker)}
                    >
                      {saving === broker.id ? 'Saving...' : 'Save Credentials'}
                    </button>
                    {conf && (
                      <button
                        className="btn-danger text-xs py-1.5 px-3 disabled:opacity-50"
                        disabled={deleting === broker.id}
                        onClick={() => handleDelete(broker)}
                      >
                        {deleting === broker.id ? 'Deleting...' : 'Delete'}
                      </button>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
