import type { Config } from 'tailwindcss';
import typography from '@tailwindcss/typography';

export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        navy: '#172B4D',
        ecblue: '#2878C7',
        ecteal: '#22A58A',
        bg: '#F3F6FA'
      },
      fontFamily: {
        sans: ['Cairo', 'IBM Plex Sans Arabic', 'Segoe UI', 'Tahoma', 'sans-serif']
      }
    }
  },
  plugins: [typography]
} satisfies Config;
