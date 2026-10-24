(() => {
  const root = document.documentElement;
  const toggle = document.getElementById('themeToggle');
  const current = () => root.getAttribute('data-theme') || 'light';
  const updateLabel = () => { if (toggle) toggle.textContent = current() === 'dark' ? '☀ المظهر الفاتح' : '◐ المظهر الداكن'; };
  updateLabel();
  if (toggle) toggle.addEventListener('click', () => {
    const next = current() === 'dark' ? 'light' : 'dark';
    root.setAttribute('data-theme', next);
    try { localStorage.setItem('it-desk-theme', next); } catch (_) {}
    updateLabel();
  });

  const search = document.getElementById('ticketSearch');
  const status = document.getElementById('statusFilter');
  const rows = Array.from(document.querySelectorAll('#ticketsTable .ticket-row'));
  const noResults = document.getElementById('noResults');
  function filterRows() {
    if (!rows.length) return;
    const term = (search?.value || '').trim().toLocaleLowerCase();
    const wantedStatus = status?.value || '';
    let visible = 0;
    rows.forEach((row) => {
      const text = (row.dataset.search || row.textContent || '').toLocaleLowerCase();
      const matches = text.includes(term) && (!wantedStatus || row.dataset.status === wantedStatus);
      row.hidden = !matches;
      if (matches) visible++;
    });
    if (noResults) noResults.hidden = visible !== 0;
  }
  search?.addEventListener('input', filterRows);
  status?.addEventListener('change', filterRows);
})();
