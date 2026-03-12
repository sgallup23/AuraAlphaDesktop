import { invoke } from '@tauri-apps/api/core';

const TIER_ORDER = ['free', 'starter', 'professional', 'power_user', 'institutional', 'developer'];
const TIER_ALIASES = {
  starter_99: 'starter',
  active_trader_299: 'professional',
  power_user_pro_599: 'power_user',
  elite_997: 'institutional',
};

export async function getUser() {
  try {
    const auth = await invoke('load_auth_token');
    if (auth?.user) {
      return typeof auth.user === 'string' ? JSON.parse(auth.user) : auth.user;
    }
    return null;
  } catch { return null; }
}

export function getUserTier(user) {
  const raw = user?.subscription_tier || user?.tier || 'free';
  return TIER_ALIASES[raw] || raw;
}

export function hasTier(user, required) {
  const userLevel = TIER_ORDER.indexOf(getUserTier(user));
  const requiredLevel = TIER_ORDER.indexOf(required);
  return userLevel >= requiredLevel;
}

export function hasRole(user, required) {
  return user?.role === required || user?.role === 'admin';
}

export function tierLabel(tier) {
  const labels = { free: 'Free', starter: 'Starter', professional: 'Professional', power_user: 'Power User', institutional: 'Institutional', developer: 'Developer' };
  return labels[tier] || tier;
}
