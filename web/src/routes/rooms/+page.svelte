<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Room } from '$lib/api/people';
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
  import EntityDetail from '$lib/components/EntityDetail.svelte';
  import { count } from '$lib/format';
  import type { Column } from '$lib/table';

  let items = $state<Room[]>([]);
  let loading = $state(true);
  let error = $state('');

  let showForm = $state(false);
  let code = $state('');
  let name = $state('');
  let capacity = $state(30);
  let description = $state('');
  let saving = $state(false);
  let formError = $state('');

  let detailId = $state<number | null>(null);
  let search = $state('');

  const filtered = $derived(
    search.trim() === ''
      ? items
      : items.filter(
          (r) =>
            r.name.includes(search.trim()) ||
            r.code.toLowerCase().includes(search.trim().toLowerCase())
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
      items = await peopleApi.rooms.list(token, 1);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل القاعات';
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
      await invoke('rooms_create', {
        token,
        branch_id: 1,
        code: code.trim(),
        name: name.trim(),
        capacity: Number(capacity),
        description: description.trim()
      });
      toast.ok(`تمت إضافة قاعة «${name.trim()}»`);
      showForm = false;
      code = '';
      name = '';
      capacity = 30;
      description = '';
      await load();
    } catch (err) {
      formError = (err as Error)?.message ?? 'تعذّر إضافة القاعة';
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

{#snippet detailActions(r: Room)}
  <Button variant="ghost" size="sm" onclick={() => (detailId = r.id)}>عرض</Button>
{/snippet}

{#snippet statusCell(r: Room)}
  <Badge status={r.status} />
{/snippet}

{#snippet capacityCell(r: Room)}
  <span class="ec-num">{count(r.capacity)}</span>
{/snippet}

<PageHeader
  title="القاعات"
  subtitle={`${items.length} قاعة — السعة تُستخدم في قواعد التسجيل وفحص الجدول`}
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={() => (showForm = true)}>+ إضافة قاعة</Button>
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
        aria-label="بحث في القاعات"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'code', label: 'الكود', num: true, width: '110px' },
        { key: 'name', label: 'اسم القاعة' },
        { key: 'capacity', label: 'السعة', num: true, render: capacityCell },
        { key: 'description', label: 'الوصف', format: (v) => String(v || '—') },
        { key: 'status', label: 'الحالة', render: statusCell, width: '130px' }
      ]}
      rows={filtered}
      rowKey={(r) => r.id}
      loading={loading}
      emptyTitle="لا توجد قاعات"
      emptyHint="أضف قاعاتك ليتمكن الجدول والتسجيل من التحقق من السعة."
      emptyIcon="▦"
      actions={detailActions}
    />
  </div>
</Card>

<Modal
  open={showForm}
  title="إضافة قاعة جديدة"
  onClose={() => (showForm = false)}
>
  <form class="space-y-4" onsubmit={create}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="الكود" required hint="مثال: R-01">
        <input class="ec-input" dir="ltr" bind:value={code} required placeholder="R-01" />
      </Field>
      <Field label="اسم القاعة" required>
        <input class="ec-input" bind:value={name} required placeholder="القاعة الكبرى" />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="السعة (عدد المقاعد)" required>
        <input
          class="ec-input ec-num"
          type="number"
          min="1"
          bind:value={capacity}
          dir="ltr"
          required
        />
      </Field>
      <Field label="الحالة الافتراضية">
        <input class="ec-input" value="متاحة" disabled />
      </Field>
    </div>
    <Field label="الوصف">
      <textarea class="ec-textarea" bind:value={description} placeholder="موقع القاعة، معداتها…"></textarea>
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showForm = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving}>حفظ القاعة</Button>
    </div>
  </form>
</Modal>

{#if detailId !== null}
  <EntityDetail kind="room" id={detailId} onClose={() => (detailId = null)} />
{/if}
