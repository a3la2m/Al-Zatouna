<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Subject } from '$lib/api/people';
  import { invoke } from '$lib/api';
  import { toast } from '$lib/toasts';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import DataTable from '$lib/components/DataTable.svelte';

  let items = $state<Subject[]>([]);
  let loading = $state(true);
  let error = $state('');

  let showForm = $state(false);
  let code = $state('');
  let name = $state('');
  let description = $state('');
  let saving = $state(false);
  let formError = $state('');

  let search = $state('');

  const filtered = $derived(
    search.trim() === ''
      ? items
      : items.filter(
          (s) =>
            s.name.includes(search.trim()) ||
            s.code.toLowerCase().includes(search.trim().toLowerCase())
        )
  );

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      items = await peopleApi.subjects.list(token, false);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل المواد';
    } finally {
      loading = false;
    }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    formError = '';
    saving = true;
    try {
      await invoke('subjects_create', {
        token,
        code: code.trim(),
        name: name.trim(),
        description: description.trim()
      });
      toast.ok(`تمت إضافة مادة «${name.trim()}»`);
      showForm = false;
      code = '';
      name = '';
      description = '';
      await load();
    } catch (err) {
      formError = (err as Error)?.message ?? 'تعذّر إضافة المادة';
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

{#snippet statusCell(s: Subject)}
  <Badge status={s.is_active ? 'active' : 'inactive'} />
{/snippet}

<PageHeader
  title="المواد"
  subtitle={`${items.length} مادة — تُستخدم في الدورات والتقارير الأكاديمية`}
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={() => (showForm = true)}>+ إضافة مادة</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<Card pad={false}>
  <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
    <div class="ec-toolbar" style="margin-bottom:0">
      <input
        class="ec-input grow"
        type="search"
        placeholder="ابحث بالاسم أو الكود…"
        bind:value={search}
        aria-label="بحث في المواد"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'code', label: 'الكود', num: true, width: '110px' },
        { key: 'name', label: 'اسم المادة' },
        { key: 'description', label: 'الوصف', format: (v) => String(v || '—') },
        { key: 'is_active', label: 'الحالة', render: statusCell, width: '120px' }
      ]}
      rows={filtered}
      rowKey={(s) => s.id}
      loading={loading}
      emptyTitle="لا توجد مواد"
      emptyHint="أضف أول مادة لتتمكن من إنشاء الدورات."
      emptyIcon="✔"
    />
  </div>
</Card>

<Modal
  open={showForm}
  title="إضافة مادة جديدة"
  subtitle="الكود فريد ويُستخدم في التقارير."
  onClose={() => (showForm = false)}
>
  <form class="space-y-4" onsubmit={create}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="الكود" required hint="مثال: MATH101">
        <input class="ec-input" dir="ltr" bind:value={code} required placeholder="MATH101" />
      </Field>
      <Field label="اسم المادة" required>
        <input class="ec-input" bind:value={name} required placeholder="الرياضيات" />
      </Field>
    </div>
    <Field label="الوصف">
      <textarea class="ec-textarea" bind:value={description} placeholder="وصف مختصر…"></textarea>
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showForm = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving}>حفظ المادة</Button>
    </div>
  </form>
</Modal>
