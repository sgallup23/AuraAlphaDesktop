import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { theme, styles } from '../theme';

// Risk tier color mapping
const tierColor = {
  official: theme.green,
  official_gateway: theme.amber,
  unofficial: theme.red,
};

const tierLabel = {
  official: 'Official API',
  official_gateway: 'Gateway Required',
  unofficial: 'Unofficial',
};

export default function BrokerSetupPage() {
  const [brokers, setBrokers] = useState([]);
  const [configured, setConfigured] = useState([]);
  const [expandedId, setExpandedId] = useState(null);
  const [formData, setFormData] = useState({});
  const [waiverAccepted, setWaiverAccepted] = useState({});
  const [saving, setSaving] = useState(null);
  const [deleting, setDeleting] = useState(null);
  const [message, setMessage] = useState(null);

  const loadData = useCallback(async () => {
    try {
      const [allBrokers, configuredList] = await Promise.all([
        invoke('get_available_brokers'),
        invoke('list_configured_brokers'),
      ]);
      setBrokers(allBrokers);
      setConfigured(configuredList);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to load brokers: ' + err });
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const toggleExpand = (id) => {
    setExpandedId((prev) => (prev === id ? null : id));
    setMessage(null);
  };

  const handleFieldChange = (brokerId, fieldName, value) => {
    setFormData((prev) => ({
      ...prev,
      [brokerId]: {
        ...(prev[brokerId] || {}),
        [fieldName]: value,
      },
    }));
  };

  const handleSave = async (broker) => {
    if (broker.waiver_required && !waiverAccepted[broker.id]) {
      setMessage({ type: 'error', text: 'You must accept the waiver to proceed.' });
      return;
    }

    setSaving(broker.id);
    setMessage(null);
    try {
      const credentials = formData[broker.id] || {};

      // Validate required fields
      const missingFields = broker.credential_fields
        .filter((f) => f.required && !credentials[f.name]?.trim())
        .map((f) => f.label);

      if (missingFields.length > 0) {
        setMessage({
          type: 'error',
          text: 'Missing required fields: ' + missingFields.join(', '),
        });
        setSaving(null);
        return;
      }

      await invoke('configure_broker', { broker: broker.id, credentials });
      setMessage({ type: 'success', text: `${broker.name} credentials saved successfully.` });

      // Refresh configured list
      const updated = await invoke('list_configured_brokers');
      setConfigured(updated);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to save: ' + err });
    } finally {
      setSaving(null);
    }
  };

  const handleDelete = async (broker) => {
    setDeleting(broker.id);
    setMessage(null);
    try {
      await invoke('delete_broker_credentials', { broker: broker.id });
      setMessage({ type: 'success', text: `${broker.name} credentials deleted.` });

      // Clear form data
      setFormData((prev) => {
        const next = { ...prev };
        delete next[broker.id];
        return next;
      });

      // Refresh
      const updated = await invoke('list_configured_brokers');
      setConfigured(updated);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to delete: ' + err });
    } finally {
      setDeleting(null);
    }
  };

  const isConfigured = (id) => configured.includes(id);

  // Sort: configured first, then by tier (official > gateway > unofficial)
  const tierOrder = { official: 0, official_gateway: 1, unofficial: 2 };
  const sortedBrokers = [...brokers].sort((a, b) => {
    const aConf = isConfigured(a.id) ? 0 : 1;
    const bConf = isConfigured(b.id) ? 0 : 1;
    if (aConf !== bConf) return aConf - bConf;
    return (tierOrder[a.risk_tier] || 9) - (tierOrder[b.risk_tier] || 9);
  });

  return (
    <div style={styles.page}>
      <div style={{ maxWidth: 900, margin: '0 auto' }}>
        {/* Header */}
        <div style={{ marginBottom: 24 }}>
          <h1 style={styles.h1}>Broker Setup</h1>
          <p style={styles.subtitle}>
            Configure broker API credentials. Stored encrypted on your local machine.
          </p>
        </div>

        {/* Summary bar */}
        <div
          style={{
            ...styles.card,
            display: 'flex',
            gap: 24,
            padding: '14px 20px',
          }}
        >
          <span style={{ fontSize: 13, color: theme.textMuted }}>
            <strong style={{ color: theme.text }}>{brokers.length}</strong> brokers available
          </span>
          <span style={{ fontSize: 13, color: theme.textMuted }}>
            <strong style={{ color: theme.green }}>{configured.length}</strong> configured
          </span>
          <span style={{ fontSize: 13, color: theme.textMuted }}>
            <strong style={{ color: theme.textMuted }}>
              {brokers.length - configured.length}
            </strong>{' '}
            not configured
          </span>
        </div>

        {/* Message banner */}
        {message && (
          <div
            style={{
              padding: '10px 16px',
              marginBottom: 16,
              borderRadius: 8,
              fontSize: 13,
              background:
                message.type === 'error' ? theme.red + '1A' : theme.green + '1A',
              color: message.type === 'error' ? theme.red : theme.green,
              border: `1px solid ${message.type === 'error' ? theme.red + '33' : theme.green + '33'}`,
            }}
          >
            {message.text}
          </div>
        )}

        {/* Broker cards */}
        {sortedBrokers.map((broker) => {
          const expanded = expandedId === broker.id;
          const configured_ = isConfigured(broker.id);
          const color = tierColor[broker.risk_tier] || theme.textMuted;

          return (
            <div
              key={broker.id}
              style={{
                ...styles.card,
                borderLeftWidth: 3,
                borderLeftColor: configured_ ? theme.green : color,
                cursor: 'pointer',
                transition: 'border-color 0.2s',
              }}
            >
              {/* Card header (clickable) */}
              <div
                style={styles.flexBetween}
                onClick={() => toggleExpand(broker.id)}
              >
                <div style={styles.flexRow}>
                  <span
                    style={styles.dot(configured_ ? theme.green : theme.border)}
                  />
                  <span style={{ fontSize: 15, fontWeight: 600 }}>
                    {broker.name}
                  </span>
                  <span style={styles.badge(color)}>
                    {tierLabel[broker.risk_tier] || broker.risk_tier}
                  </span>
                  {configured_ && (
                    <span style={styles.badge(theme.green)}>Configured</span>
                  )}
                </div>
                <span
                  style={{
                    fontSize: 18,
                    color: theme.textMuted,
                    transform: expanded ? 'rotate(180deg)' : 'rotate(0deg)',
                    transition: 'transform 0.2s',
                    userSelect: 'none',
                  }}
                >
                  ▾
                </span>
              </div>

              {/* Description line */}
              <p
                style={{
                  fontSize: 13,
                  color: theme.textMuted,
                  margin: '8px 0 0',
                  lineHeight: 1.4,
                }}
              >
                {broker.description}
              </p>

              {/* Feature pills */}
              <div style={{ display: 'flex', gap: 8, marginTop: 8, flexWrap: 'wrap' }}>
                <span style={featurePill}>
                  Commission: {broker.commission === 'free' ? 'Free' : broker.commission}
                </span>
                {broker.fractional && (
                  <span style={featurePill}>Fractional Shares</span>
                )}
                {broker.paper_trading && (
                  <span style={featurePill}>Paper Trading</span>
                )}
              </div>

              {/* Expanded form */}
              {expanded && (
                <div
                  style={{
                    marginTop: 16,
                    paddingTop: 16,
                    borderTop: `1px solid ${theme.border}`,
                  }}
                  onClick={(e) => e.stopPropagation()}
                >
                  {/* Waiver */}
                  {broker.waiver_required && broker.waiver_text && (
                    <div
                      style={{
                        background: (broker.risk_tier === 'unofficial' ? theme.red : theme.amber) + '0D',
                        border: `1px solid ${(broker.risk_tier === 'unofficial' ? theme.red : theme.amber) + '33'}`,
                        borderRadius: 8,
                        padding: 14,
                        marginBottom: 16,
                      }}
                    >
                      <p
                        style={{
                          fontSize: 12,
                          lineHeight: 1.6,
                          color: broker.risk_tier === 'unofficial' ? theme.red : theme.amber,
                          margin: 0,
                        }}
                      >
                        {broker.waiver_text}
                      </p>
                      <label
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: 8,
                          marginTop: 12,
                          fontSize: 13,
                          color: theme.text,
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={waiverAccepted[broker.id] || false}
                          onChange={(e) =>
                            setWaiverAccepted((prev) => ({
                              ...prev,
                              [broker.id]: e.target.checked,
                            }))
                          }
                          style={{ accentColor: theme.blue }}
                        />
                        I acknowledge and accept the risks described above
                      </label>
                    </div>
                  )}

                  {/* Credential fields */}
                  <div style={styles.grid2}>
                    {broker.credential_fields.map((field) => (
                      <div key={field.name} style={styles.fieldGroup}>
                        <label style={styles.label}>
                          {field.label}
                          {field.required && (
                            <span style={{ color: theme.red, marginLeft: 4 }}>*</span>
                          )}
                        </label>
                        <input
                          type={field.field_type === 'password' ? 'password' : 'text'}
                          placeholder={field.placeholder}
                          value={(formData[broker.id] || {})[field.name] || ''}
                          onChange={(e) =>
                            handleFieldChange(broker.id, field.name, e.target.value)
                          }
                          style={styles.input}
                          onFocus={(e) =>
                            (e.target.style.borderColor = theme.blue)
                          }
                          onBlur={(e) =>
                            (e.target.style.borderColor = theme.border)
                          }
                        />
                      </div>
                    ))}
                  </div>

                  {/* Action buttons */}
                  <div style={{ display: 'flex', gap: 12, marginTop: 8 }}>
                    <button
                      style={{
                        ...styles.button,
                        opacity:
                          saving === broker.id ||
                          (broker.waiver_required && !waiverAccepted[broker.id])
                            ? 0.5
                            : 1,
                      }}
                      disabled={
                        saving === broker.id ||
                        (broker.waiver_required && !waiverAccepted[broker.id])
                      }
                      onClick={() => handleSave(broker)}
                    >
                      {saving === broker.id ? 'Saving...' : 'Save Credentials'}
                    </button>
                    {configured_ && (
                      <button
                        style={{
                          ...styles.buttonDanger,
                          opacity: deleting === broker.id ? 0.5 : 1,
                        }}
                        disabled={deleting === broker.id}
                        onClick={() => handleDelete(broker)}
                      >
                        {deleting === broker.id ? 'Deleting...' : 'Delete Credentials'}
                      </button>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}

        {brokers.length === 0 && (
          <div
            style={{
              ...styles.card,
              textAlign: 'center',
              color: theme.textMuted,
              padding: 40,
            }}
          >
            Loading brokers...
          </div>
        )}
      </div>
    </div>
  );
}

const featurePill = {
  display: 'inline-block',
  padding: '2px 8px',
  borderRadius: 4,
  fontSize: 11,
  background: theme.surfaceAlt,
  color: theme.textMuted,
  border: `1px solid ${theme.border}`,
};
