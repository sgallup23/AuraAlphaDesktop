import { NavLink } from 'react-router-dom';
import { theme } from '../theme';

const links = [
  { to: '/', label: 'Dashboard' },
  { to: '/brokers', label: 'Brokers' },
  { to: '/bots', label: 'Bot Manager' },
];

const navStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 0,
  background: theme.surface,
  borderBottom: `1px solid ${theme.border}`,
  padding: '0 24px',
  height: 48,
  position: 'sticky',
  top: 0,
  zIndex: 100,
};

const logoStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 10,
  marginRight: 32,
  textDecoration: 'none',
  color: theme.text,
};

const logoIconStyle = {
  width: 28,
  height: 28,
  borderRadius: 7,
  background: `linear-gradient(135deg, ${theme.blue} 0%, ${theme.purple} 50%, ${theme.green} 100%)`,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  fontSize: 14,
  fontWeight: 700,
  color: '#fff',
};

const baseLinkStyle = {
  padding: '12px 16px',
  fontSize: 14,
  fontWeight: 500,
  color: theme.textMuted,
  textDecoration: 'none',
  borderBottom: '2px solid transparent',
  transition: 'color 0.2s, border-color 0.2s',
};

const activeLinkStyle = {
  ...baseLinkStyle,
  color: theme.text,
  borderBottomColor: theme.blue,
};

export default function NavBar() {
  return (
    <nav style={navStyle}>
      <div style={logoStyle}>
        <div style={logoIconStyle}>A</div>
        <span style={{ fontSize: 15, fontWeight: 600, letterSpacing: -0.3 }}>
          Aura Alpha
        </span>
      </div>

      {links.map((link) => (
        <NavLink
          key={link.to}
          to={link.to}
          style={({ isActive }) => (isActive ? activeLinkStyle : baseLinkStyle)}
        >
          {link.label}
        </NavLink>
      ))}
    </nav>
  );
}
