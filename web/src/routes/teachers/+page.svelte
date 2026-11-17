<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Teacher } from '$lib/api/people';
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

  let items = $state<Teacher[]>([]);
  let loading = $state(true);
  let error = $state('');

  let showForm = $state(false);
  let editing = $state<Teacher | null>(null);
  let fullName = $state('');
  let phone = $state('');
  let email = $state('');
  let specialty = $state('');
  let qualifications = $state('');
  let address = $state('');
  let hireDate = $state('');
  let notes = $state('');
  let saving = $state(false);
  let formError = $state('');

  let detailId = $state<number | null>(null);
  let search = $state('');

  const filtered = $derived(
    search.trim() === ''
      ? items
      : items.filter(
          (t) =>
            t.full_name.includes(search.trim()) ||
            t.specialty.includes(search.trim()) ||
            t.phone.includes(search.trim())
        )
  );

  function openCreate() {
    editing = null;
    fullName = '';
    phone = '';
    email = '';
    specialty = '';
    qualifications = '';
    address = '';
    hireDate = '';
    notes = '';
    formError = '';
    showForm = true;
  }

  function openEdit(t: Teacher) {
    editing = t;
    fullName = t.full_name;
    phone = t.phone;
    email = t.email;
    specialty = t.specialty;
    qualifications = t.qualifications;
    address = t.address;
    hireDate = t.hire_date;
    notes = t.notes;
    formError = '';
    showForm = true;
  }

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      items = await peopleApi.teachers.list(token, 1);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل المدرسين';
    } finally {
      loading = false;
    }
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    formError = '';
    saving = true;
    try {
      if (editing) {
        await peopleApi.teachers.edit(token, editing.id, {
          full_name: fullName,
          phone,
          email,
          specialty,
          qualifications,
          address,
          hire_date: hireDate,
          notes
        });
        toast.ok('تم تحديث بيانات المدرس');
      } else {
        await peopleApi.teachers.create(token, {
          branch_id: 1,
          full_name: fullName,
          phone,
          email,
          specialty,
          qualifications,
          address,
          hire_date: hireDate,
          notes
        });
        toast.ok(`تمت إضافة المدرس «${fullName}»`);
      }
      showForm = false;
      await load();
    } catch (err) {
      formError = (err as Error)?.message ?? 'تعذّر الحفظ';
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

{#snippet statusCell(t: Teacher)}
  <Badge status={t.status} />
{/snippet}

<PageHeader
  title="المدرسون"
  subtitle={`${items.length} مدرس — يظهر حجم العمل لكل مدرس في تقرير «حجم عمل المدرسين»`}
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={openCreate}>+ إضافة مدرس</Button>
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
        placeholder="ابحث بالاسم أو التخصص أو الهاتف…"
        bind:value={search}
        aria-label="بحث في المدرسين"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'teacher_no', label: 'الرقم', num: true, width: '80px' },
        { key: 'full_name', label: 'الاسم الكامل' },
        { key: 'specialty', label: 'التخصص', format: (v) => String(v || '—') },
        { key: 'phone', label: 'الهاتف', num: true, format: (v) => String(v || '—') },
        { key: 'email', label: 'البريد', format: (v) => String(v || '—') },
        { key: 'status', label: 'الحالة', render: statusCell, width: '130px' }
      ]}
      rows={filtered}
      rowKey={(t) => t.id}
      loading={loading}
      emptyTitle="لا يوجد مدرسون"
      emptyHint="أضف المدرسين لربطهم بالدورات والحصص."
      emptyIcon="👔"
    >
      {#snippet actions(t: Teacher)}
        <Button variant="ghost" size="sm" onclick={() => (detailId = t.id)}>عرض</Button>
        <Button variant="ghost" size="sm" onclick={() => openEdit(t)}>تعديل</Button>
      {/snippet}
    </DataTable>
  </div>
</Card>

<Modal
  open={showForm}
  title={editing ? 'تعديل بيانات المدرس' : 'إضافة مدرس جديد'}
  subtitle={editing ? editing.full_name : 'الاسم والتخصص يكفيان للبدء.'}
  onClose={() => (showForm = false)}
  width="640px"
>
  <form class="space-y-4" onsubmit={save}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="الاسم الكامل" required>
        <input class="ec-input" bind:value={fullName} required />
      </Field>
      <Field label="التخصص">
        <input class="ec-input" bind:value={specialty} placeholder="رياضيات" />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="الهاتف">
        <input class="ec-input" dir="ltr" bind:value={phone} placeholder="05x xxxxxxx" />
      </Field>
      <Field label="البريد الإلكتروني">
        <input class="ec-input" type="email" dir="ltr" bind:value={email} />
      </Field>
    </div>
    <div class="ec-form-grid">
      <Field label="المؤهلات">
        <input class="ec-input" bind:value={qualifications} placeholder="بكالوريوس رياضيات" />
      </Field>
      <Field label="تاريخ التوظيف">
        <input class="ec-input" type="date" dir="ltr" bind:value={hireDate} />
      </Field>
    </div>
    <Field label="العنوان">
      <input class="ec-input" bind:value={address} />
    </Field>
    <Field label="ملاحظات">
      <textarea class="ec-textarea" bind:value={notes}></textarea>
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showForm = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving}>
        {editing ? 'حفظ التعديلات' : 'إضافة المدرس'}
      </Button>
    </div>
  </form>
</Modal>

{#if detailId !== null}
  <EntityDetail kind="teacher" id={detailId} onClose={() => (detailId = null)} />
{/if}
