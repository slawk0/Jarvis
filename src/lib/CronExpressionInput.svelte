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
          <option value="*/5">*/5</option>
          <option value="*/15">*/15</option>
          <option value="0">0 (on hour)</option>
          <option value="30">30</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-hour">Hour</label>
        <select id="gen-hour" bind:value={presetHours} onchange={updateFromPresets}>
          <option value="*">Every (*)</option>
          <option value="*/2">*/2</option>
          <option value="0">0 (midnight)</option>
          <option value="12">12 (noon)</option>
          <option value="2">2 (2 AM)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-day">Day</label>
        <select id="gen-day" bind:value={presetDays} onchange={updateFromPresets}>
          <option value="*">Every (*)</option>
          <option value="1">1st</option>
          <option value="15">15th</option>
          <option value="*/2">*/2</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-month">Month</label>
        <select id="gen-month" bind:value={presetMonths} onchange={updateFromPresets}>
          <option value="*">Every (*)</option>
          <option value="1">Jan</option>
          <option value="*/3">*/3</option>
        </select>
      </div>

      <div class="form-group">
        <label for="gen-dow">Weekday</label>
        <select id="gen-dow" bind:value={presetDayOfWeek} onchange={updateFromPresets}>
          <option value="*">Every (*)</option>
          <option value="1-5">Mon–Fri</option>
          <option value="0,6">Sat–Sun</option>
          <option value="1">Mon</option>
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
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 8px;
  }
</style>
