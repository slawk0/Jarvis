<script lang="ts">
  // Reusable cron expression editor: a raw text field plus a visual builder.
  // Two-way bound via `value` (the five-field cron expression).
  let { value = $bindable('*/5 * * * *') } = $props();

  let presetMinutes = $state('*/5');
  let presetHours = $state('*');
  let presetDays = $state('*');
  let presetMonths = $state('*');
  let presetDayOfWeek = $state('*');

  // Keep the visual builder in sync when `value` is set externally
  // (e.g. when opening an existing schedule for editing).
  $effect(() => {
    const parts = value.trim().split(/\s+/);
    if (parts.length === 5) {
      presetMinutes = parts[0];
      presetHours = parts[1];
      presetDays = parts[2];
      presetMonths = parts[3];
      presetDayOfWeek = parts[4];
    }
  });

  function updateFromPresets() {
    value = `${presetMinutes} ${presetHours} ${presetDays} ${presetMonths} ${presetDayOfWeek}`;
  }
</script>

<div class="cron-input">
  <div class="form-group">
    <label for="cron-expr">Cron expression</label>
    <input id="cron-expr" type="text" placeholder="* * * * *" bind:value />
  </div>

  <div class="cron-generator glass">
    <h4>Visual schedule builder</h4>
    <div class="generator-grid">
      <div class="form-group">
        <label for="gen-min">Minute</label>
        <select id="gen-min" bind:value={presetMinutes} onchange={updateFromPresets}>
          <option value="*">Every (*)</option>
          <option value="*/5">Every 5 minutes (*/5)</option>
          <option value="*/15">Every 15 minutes (*/15)</option>
          <option value="0">On the hour (0)</option>
          <option value="30">At minute 30 (30)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-hour">Hour</label>
        <select id="gen-hour" bind:value={presetHours} onchange={updateFromPresets}>
          <option value="*">Every hour (*)</option>
          <option value="*/2">Every 2 hours (*/2)</option>
          <option value="0">Midnight (00:00)</option>
          <option value="12">Noon (12:00)</option>
          <option value="2">2 AM (02:00)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-day">Day of month</label>
        <select id="gen-day" bind:value={presetDays} onchange={updateFromPresets}>
          <option value="*">Every day (*)</option>
          <option value="1">First day (1)</option>
          <option value="15">Mid-month (15)</option>
          <option value="*/2">Every other day (*/2)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-month">Month</label>
        <select id="gen-month" bind:value={presetMonths} onchange={updateFromPresets}>
          <option value="*">Every month (*)</option>
          <option value="1">January (1)</option>
          <option value="*/3">Quarterly (*/3)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-dow">Day of week</label>
        <select id="gen-dow" bind:value={presetDayOfWeek} onchange={updateFromPresets}>
          <option value="*">Every day (*)</option>
          <option value="1-5">Weekdays (Mon–Fri)</option>
          <option value="0,6">Weekend (Sat–Sun)</option>
          <option value="1">Monday (1)</option>
        </select>
      </div>
    </div>
  </div>
</div>

<style>
  .cron-input {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .cron-input .form-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .cron-input label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-weight: 400;
    text-transform: none;
    letter-spacing: normal;
  }
  .cron-input input,
  .cron-input select {
    width: 100%;
    box-sizing: border-box;
    min-width: 0;
    font-size: 0.85rem;
    padding: 6px 8px;
  }
  .cron-generator {
    padding: 12px;
    border-radius: var(--radius-md);
  }
  .cron-generator h4 {
    margin: 0 0 10px;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }
  .generator-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
</style>
