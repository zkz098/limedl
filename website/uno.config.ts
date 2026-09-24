import {
  defineConfig,
  presetUno,
  presetIcons,
  presetTypography,
  transformerDirectives,
  transformerVariantGroup,
} from 'unocss';

export default defineConfig({
  presets: [
    presetUno(),
    presetIcons({
      scale: 1.2,
      warn: true,
      extraProperties: {
        'display': 'inline-block',
        'vertical-align': 'middle',
      },
    }),
    presetTypography(),
  ],
  transformers: [
    transformerDirectives(),
    transformerVariantGroup(),
  ],
  theme: {
    colors: {
      brand: {
        dark: '#0d0f12',
        surface: '#12151a',
        card: '#181b22',
        border: '#262a33',
        borderLight: '#363c48',
        muted: '#94a3b8',
      },
      lime: {
        50: '#f7fee7',
        100: '#ecfccb',
        200: '#d9f99d',
        300: '#bef264',
        400: '#a3e635',
        500: '#84cc16',
        600: '#65a30d',
        700: '#4d7c0f',
        800: '#3f6212',
        900: '#365314',
        950: '#1a2e05',
      },
    },
  },
  shortcuts: {
    'btn-primary': 'inline-flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-medium text-black bg-lime-400 hover:bg-lime-300 transition-all duration-200 shadow-lg shadow-lime-500/20 active:scale-95',
    'btn-secondary': 'inline-flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-medium text-slate-200 bg-brand-card hover:bg-brand-surface border border-brand-border hover:border-lime-500/50 transition-all duration-200 active:scale-95',
    'glass-card': 'bg-brand-card/80 backdrop-blur-md border border-brand-border rounded-2xl p-6 transition-all duration-300 hover:border-brand-borderLight hover:shadow-xl hover:shadow-lime-500/5',
  },
});
