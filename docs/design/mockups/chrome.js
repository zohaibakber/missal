const OFFENCES = ["چوری ۳۷۹ ت پ", "ڈکیتی ۳۹۲ ت پ", "قتل عمد ۳۰۲ ت پ", "اقدام قتل ۳۲۴ ت پ", "منشیات ۹ سی", "راہزنی ۳۹۴ ت پ", "فراڈ ۴۲۰ ت پ", "اغوا ۳۶۳ ت پ", "غیر قانونی اسلحہ ۱۵ اے", "لڑائی جھگڑا ۳۳۷ ت پ", "نقب زنی ۴۵۷ ت پ", "دھوکہ دہی اور جعل سازی ۴۶۸ ت پ"];
const NAMES = ["محمد اسلم ولد اکرم سکنہ چک ۴۵", "علی رضا ولد حسن سکنہ محلہ اسلام پورہ", "عمران خان ولد شیر خان سکنہ گوجرانوالہ", "فیصل محمود ولد محمود احمد سکنہ ملتان", "ندیم عباس ولد غلام عباس سکنہ لاہور", "طارق جاوید ولد جاوید اقبال سکنہ فیصل آباد", "شاہد نواز ولد نواز علی سکنہ بہاولپور", "کامران اکبر ولد اکبر علی سکنہ سیالکوٹ", "زاہد حسین ولد حسین بخش سکنہ ساہیوال", "اظہر اقبال ولد اقبال حسین سکنہ جھنگ"];
const STATUS = { open: ["زیر التوا", "d-open"], inv: ["زیر تفتیش", "d-inv"], challan: ["چالان جمع", "d-challan"], closed: ["بند", "d-closed"] };
const ICON = (n, c) => `<i data-i="${n}"${c ? ` class="${c}"` : ''}></i>`;

function titlebar(o) {
  if (typeof o === 'string') o = { title: o };
  return `<div class="tb">
    <div class="ib${o.menuOpen ? ' on' : ''}" title="Menu">${ICON('menu')}</div>
    <div class="ib">${ICON('arrow-left')}</div>
    <div class="ib dis">${ICON('arrow-right')}</div>
    <div class="tb-title">${o.title || ''}</div>
    ${o.meta ? `<div class="tb-meta">${o.meta}</div>` : ''}
    <div class="grow"></div>
    <div class="tb-actions">${o.actions || ''}</div>
    <div class="ib" title="Search (Ctrl K)">${ICON('search')}</div>
    <div class="winctl"><span>${ICON('minus', 'i14')}</span><span>${ICON('square', 'i12')}</span><span>${ICON('x', 'i14')}</span></div>
  </div>`;
}

function navmenu() {
  const it = (icon, label, keys, on) => `<div class="mi${on ? ' hl' : ''}">${icon}<span class="grow">${label}</span>${keys}</div>`;
  const k = (...a) => `<span class="row" style="gap:3px">${a.map((x) => `<span class="kbd">${x}</span>`).join('')}</span>`;
  return `<div class="pop navpop">
    ${it(ICON('folder', 'i14'), 'FIRs', k('G', 'H'), true)}
    ${it(ICON('file-text', 'i14'), 'Templates', k('G', 'T'))}
    ${it(ICON('settings', 'i14'), 'Settings', k('G', 'S'))}
    <div class="sep"></div>
    ${it(ICON('plus', 'i14'), 'New FIR', k('N'))}
    ${it(ICON('file-up', 'i14'), 'Import Word file', k('Ctrl', 'O'))}
    <div class="sep"></div>
    ${it(ICON('keyboard', 'i14'), 'Keyboard shortcuts', k('?'))}
    ${it(ICON('sun-moon', 'i14'), 'Theme', `<span class="t12 m">Light</span>`)}
    ${it(ICON('info', 'i14'), 'About Missal', '')}
  </div>`;
}
