import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './en.json'
import tr from './tr.json'

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, tr: { translation: tr } },
  lng: (() => {
    try {
      const v = localStorage.getItem('kt.lang')
      return v ? (JSON.parse(v) as string) : navigator.language?.startsWith('tr') ? 'tr' : 'en'
    } catch {
      return 'en'
    }
  })(),
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  returnNull: false,
})

export default i18n
