/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,jsx}'],
  theme: {
    extend: {
      colors: {
        aura: {
          bg: '#0D1117',
          surface: '#161B22',
          surface2: '#1C2128',
          border: '#30363D',
          text: '#E6EDF3',
          muted: '#8B949E',
          green: '#3FB950',
          red: '#F85149',
          amber: '#D29922',
          blue: '#58A6FF',
          purple: '#BC8CFF',
          cyan: '#39D2C0',
        },
      },
      fontFamily: {
        sans: ["'Plus Jakarta Sans'", '-apple-system', 'BlinkMacSystemFont', 'system-ui', 'sans-serif'],
        mono: ["'JetBrains Mono'", "'SF Mono'", "'Fira Code'", 'monospace'],
      },
      borderRadius: {
        DEFAULT: '8px',
        lg: '12px',
        xl: '16px',
      },
    },
  },
  plugins: [require('@tailwindcss/forms')({ strategy: 'class' })],
};
