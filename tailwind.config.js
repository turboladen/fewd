/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        primary: {
          50: '#f0f5f0',
          100: '#dce8dc',
          200: '#b8d1b8',
          300: '#8fb98f',
          400: '#6da06d',
          500: '#5b8c5a',
          600: '#4a7a4a',
          700: '#3d653d',
          800: '#325032',
          900: '#273e27',
        },
        secondary: {
          50: '#fdf5f0',
          100: '#fbe8db',
          200: '#f5ccb0',
          300: '#edac82',
          400: '#e08c55',
          500: '#c47a4a',
          600: '#a5633a',
          700: '#864f2e',
          800: '#6b3f25',
          900: '#55321e',
        },
        accent: {
          50: '#fdf8ec',
          100: '#faefc9',
          200: '#f4de8e',
          300: '#edc94e',
          400: '#deb321',
          500: '#c49a1a',
          600: '#a37b14',
          700: '#825d10',
          800: '#6b4c12',
          900: '#5a3f14',
        },
        surface: '#FDFAF6',
      },
      fontFamily: {
        heading: ['Georgia', '"Times New Roman"', 'serif'],
      },
    },
  },
  plugins: [],
}
