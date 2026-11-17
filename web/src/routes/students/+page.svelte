<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { peopleApi, type Student } from '$lib/api/people';
  import { peopleExtras } from '$lib/api/app';
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
  import Tabs from '$lib/components/Tabs.svelte';
  import StatCard from '$lib/components/StatCard.svelte';
  import { count, date } from '$lib/format';

  let items = $state<Student[]>([]);
  let loading = $state(true);
  let error = $state('');

  let detailId = $state<number | null>(null);

  let statusFilter = $state('all');
  let search = $state('');

  let showForm = $state(false);
  let editing = $state<Student | null>(null);
  let fullName = $state('');
  let nationalId = $state('');
  let phone = $state('');
  let email = $state('');
  let address = $state('');
  let gender = $state('');
  let birthDate = $state('');
  let notes = $state('');
  let saving = $state(false);
  let formError = $state('');

  const TABS = [
    { id: 'all', label: 'الكل' },
    { id: 'active', label: 'نشط' },
    { id: 'frozen', label: 'مجمّد' },
    { id: 'archived', label: 'مؤرشف' }
  ];

  const filtered = $derived(
    items.filter(
      (s) =>
        (statusFilter === 'all' || s.status === statusFilter) &&
        (search.trim() === '' ||
          s.full_name.includes(search.trim()) ||
          s.phone.includes(search.trim()) ||
          s.national_id.includes(search.trim()) ||
          String(s.student_no).includes(search.trim()))
    )
  );

  const byStatus = $derived({
    all: items.length,
    active: items.filter((s) => s.status === 'active').length,
    frozen: items.filter((s) => s.status === 'frozen').length,
    archived: items.filter((s) => s.status === 'archived').length
  });

  function openCreate() {
    editing = null;
    fullName = '';
    nationalId = '';
    phone = '';
    email = '';
    address = '';
    gender = '';
    birthDate = '';
    notes = '';
    formError = '';
    showForm = true;
  }

  function openEdit(s: Student) {
    editing = s;
    fullName = s.full_name;
    nationalId = s.national_id;
    phone = s.phone;
    email = s.email;
    address = s.address;
    gender = s.gender ?? '';
    birthDate = s.birth_date;
    notes = s.notes;
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
      items = await peopleApi.students.list(token, 1, undefined, undefined, 500);
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل الطلاب';
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
      const payload = {
        full_name: fullName,
        national_id: nationalId,
        birth_date: birthDate,
        gender: gender || null,
        phone,
        email,
        address,
        notes
      };
      if (editing) {
        await peopleExtras.students.edit(token, editing.id, payload);
        toast.ok('تم تحديث بيانات الطالب');
      } else {
        await peopleApi.students.create(token, { branch_id: 1, ...payload });
        toast.ok(`تمت إضافة الطالب «${fullName}»`);
      }
      showForm = false;
      await load();
    } catch (err) {
      formError = (err as Error)?.message ?? 'تعذّر الحفظ';
    } finally {
      saving = false;
    }
  }

  async function setStatus(s: Student, status: string) {
    const token = get(sessionToken);
    if (!token) return;
    const reason =
      status === 'archived'
        ? prompt('سبب الأرشفة؟', 'انتهاء الدراسة') ?? ''
        : status === 'frozen'
          ? prompt('سبب التجميد؟', '') ?? ''
          : '';
    if ((status === 'archived' || status === 'frozen') && !reason) return;
    try {
      await peopleApi.students.setStatus(token, s.id, status, reason);
      toast.ok(`تم تحديث حالة ${s.full_name}`);
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر تحديث الحالة');
    }
  }

  onMount(load);
</script>

{#snippet statusCell(s: Student)}
  <Badge status={s.status} />
{/snippet}

{#snippet actions(s: Student)}
  <Button variant="ghost" size="sm" onclick={() => (detailId = s.id)}>عرض</Button>
  <Button variant="ghost" size="sm" onclick={() => openEdit(s)}>تعديل</Button>
  {#if s.status === 'active'}
    <Button variant="ghost" size="sm" onclick={() => setStatus(s, 'frozen')}>تجميد</Button>
    <Button variant="ghost" size="sm" onclick={() => setStatus(s, 'archived')}>أرشفة</Button>
  {:else if s.status !== 'archived'}
    <Button variant="success" size="sm" onclick={() => setStatus(s, 'active')}>تفعيل</Button>
  {/if}
{/snippet}

<PageHeader
  title="الطلاب"
  subtitle={`${items.length} طالب مسجّل — البطاقة والتسجيل والحضور مرتبطة برقم الطالب`}
>
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
  <Button variant="primary" size="sm" onclick={openCreate}>+ إضافة طالب</Button>
</PageHeader>

{#if error}
  <div class="mb-4"><Alert kind="error" onClose={() => (error = '')}>{error}</Alert></div>
{/if}

<div class="ec-kpi-row mb-4">
  <StatCard label="إجمالي الطلاب" value={count(items.length)} icon="👤" accent="var(--ec-blue)" />
  <StatCard
    label="نشطون"
    value={count(byStatus.active)}
    icon="✓"
    accent="var(--ec-teal)"
    onclick={() => (statusFilter = 'active')}
  />
  <StatCard
    label="مجمّدون"
    value={count(byStatus.frozen)}
    icon="⏸"
    accent="var(--ec-amber)"
    onclick={() => (statusFilter = 'frozen')}
  />
  <StatCard
    label="مؤرشفون"
    value={count(byStatus.archived)}
    icon="📦"
    accent="#8894ab"
    onclick={() => (statusFilter = 'archived')}
  />
</div>

<Card pad={false}>
  <div class="border-b px-5 py-4" style="border-color:var(--ec-border)">
    <div class="ec-toolbar" style="margin-bottom:0">
      <Tabs items={TABS} bind:value={statusFilter} />
      <input
        class="ec-input grow"
        type="search"
        placeholder="ابحث بالاسم أو الهاتف أو الرقم الوطني أو رقم الطالب…"
        bind:value={search}
        aria-label="بحث في الطلاب"
      />
      <span class="ec-muted text-[13px]">{filtered.length} نتيجة</span>
    </div>
  </div>

  <div class="p-4">
    <DataTable
      columns={[
        { key: 'student_no', label: 'الرقم', num: true, width: '80px' },
        { key: 'full_name', label: 'الاسم الكامل' },
        { key: 'national_id', label: 'الرقم الوطني', num: true, format: (v) => String(v || '—') },
        { key: 'phone', label: 'الهاتف', num: true, format: (v) => String(v || '—') },
        { key: 'gender', label: 'الجنس', format: (v) => (v === 'M' ? 'ذكر' : v === 'F' ? 'أنثى' : '—') },
        { key: 'registered_at', label: 'تاريخ التسجيل', format: (v) => date(String(v)) },
        { key: 'status', label: 'الحالة', render: statusCell, width: '130px' }
      ]}
      rows={filtered}
      rowKey={(s) => s.id}
      loading={loading}
      emptyTitle="لا يوجد طلاب"
      emptyHint="أضف أول طالب لتتمكن من التسجيل في الدورات وتسجيل الحضور."
      emptyIcon="👤"
      actions={actions}
    />
  </div>
</Card>

<Modal
  open={showForm}
  title={editing ? 'تعديل بيانات الطالب' : 'إضافة طالب جديد'}
  subtitle={editing ? `رقم الطالب ${editing.student_no}` : 'الاسم الكامل مطلوب، وبقية الحقول اختيارية.'}
  onClose={() => (showForm = false)}
  width="660px"
>
  <form class="space-y-4" onsubmit={save}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="الاسم الكامل" required>
        <input class="ec-input" bind:value={fullName} required />
      </Field>
      <Field label="الرقم الوطني">
        <input class="ec-input" dir="ltr" bind:value={nationalId} />
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
      <Field label="تاريخ الميلاد">
        <input class="ec-input" type="date" dir="ltr" bind:value={birthDate} />
      </Field>
      <Field label="الجنس">
        <select class="ec-select" bind:value={gender}>
          <option value="">—</option>
          <option value="M">ذكر</option>
          <option value="F">أنثى</option>
        </select>
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
        {editing ? 'حفظ التعديلات' : 'إضافة الطالب'}
      </Button>
    </div>
  </form>
</Modal>

{#if detailId !== null}
  <EntityDetail kind="student" id={detailId} onClose={() => (detailId = null)} />
{/if}
