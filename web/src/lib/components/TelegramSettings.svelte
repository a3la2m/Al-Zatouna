<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import {
    telegramApi, type TelegramSettings, type TelegramLink,
    type TelegramMessage, type TelegramStats, serviceRequestsApi, type ServiceRequest
  } from '$lib/api/app';
  import { peopleApi, type Student, type Teacher } from '$lib/api/people';
  import { toast } from '$lib/toasts';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import type { Column } from '$lib/table';

  let cfg = $state<TelegramSettings>({});
  let links = $state<TelegramLink[]>([]);
  let messages = $state<TelegramMessage[]>([]);
  let stats = $state<TelegramStats>({ links: 0, messages: 0, notifications: 0 });
  let loading = $state(true);
  let error = $state('');
  let saving = $state(false);
  let testing = $state(false);
  let testResult = $state('');

  let showLog = $state(false);
  let logSearch = $state('');
  let logKind = $state('');
  let showBroadcast = $state(false);
  let broadcastText = $state('');
  let broadcastTarget = $state('all');
  let sending = $state(false);
  let showLink = $state(false);
  let linkChat = $state('');
  let linkType = $state('student');
  let linkPerson = $state(0);
  let linkPhone = $state('');
  let students = $state<Student[]>([]);
  let teachers = $state<Teacher[]>([]);
  let linkSaving = $state(false);
  let serviceRequests = $state<ServiceRequest[]>([]);
  let showRequests = $state(false);

  const TOGGLES = [
    { key: 'tg_enabled', label: 'تفعيل البوت', hint: 'تشغيل استقبال أوامر البوت وإرسال الإشعارات' },
    { key: 'tg_queries_enabled', label: 'السماح بالاستعلامات', hint: 'الطلاب والمدرسون يمكنهم الاستعلام عن جداولهم ورسومهم' },
    { key: 'tg_notify_payment', label: 'إشعارات الدفع', hint: 'سندات القبض والمدفوعات' },
    { key: 'tg_notify_attendance', label: 'إشعارات الحضور', hint: 'تسجيل الحضور والغياب' },
    { key: 'tg_notify_session', label: 'تذكير الحصص', hint: 'تذكير قبل الحصص بـ 30 دقيقة' },
    { key: 'tg_notify_expense', label: 'إشعارات المصروفات', hint: 'سندات الصرف والتسويات' }
  ];

  const linkCols: Column<TelegramLink>[] = [
    { key: 'person_name', label: 'الشخص' },
    { key: 'person_type', label: 'النوع', format: (v) => v === 'student' ? 'طالب' : v === 'teacher' ? 'مدرس' : 'إدارة' },
    { key: 'phone', label: 'الهاتف', num: true },
    { key: 'chat_id', label: 'معرّف المحادثة', num: true },
    { key: 'active', label: 'الحالة' }
  ];
  const msgCols: Column<TelegramMessage>[] = [
    { key: 'created_at', label: 'الوقت', format: (v) => String(v).slice(0, 16).replace('T', ' ') },
    { key: 'person_name', label: 'الشخص' },
    { key: 'direction', label: 'الاتجاه', format: (v) => v === 'in' ? 'وارد' : 'صادر' },
    { key: 'kind', label: 'النوع' },
    { key: 'body', label: 'الرسالة' }
  ];

  async function load() {
    const token = get(sessionToken);
    if (!token) { loading = false; return; }
    loading = true; error = '';
    try {
      const [c, l, s, r] = await Promise.all([
        telegramApi.settings(token), telegramApi.links(token), telegramApi.stats(token),
        serviceRequestsApi.list(token)
      ]);
      cfg = c; links = l; stats = s; serviceRequests = r;
    } catch (e) { error = (e as any)?.message ?? 'تعذّر تحميل إعدادات التلجرام'; }
    finally { loading = false; }
  }

  async function save() {
    const token = get(sessionToken);
    if (!token) return;
    saving = true; error = '';
    try {
      await telegramApi.saveSettings(token, cfg);
      toast.ok('تم حفظ إعدادات التلجرام');
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر الحفظ'); }
    finally { saving = false; }
  }

  async function test() {
    const token = get(sessionToken);
    if (!token) return;
    // save token first
    await save();
    testing = true; testResult = '';
    try { testResult = await telegramApi.test(token); toast.ok(testResult); }
    catch (e) { testResult = (e as any)?.message ?? 'فشل الاختبار'; toast.error(testResult); }
    finally { testing = false; }
  }

  async function openLog() {
    showLog = true;
    await loadMessages();
  }

  async function loadMessages() {
    const token = get(sessionToken);
    if (!token) return;
    try {
      messages = await telegramApi.messages(token, undefined, logKind || undefined, logSearch || undefined, 400);
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر تحميل الرسائل'); }
  }

  async function doBroadcast(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    sending = true;
    try {
      const n = await telegramApi.broadcast(token, broadcastText, broadcastTarget);
      toast.ok(`تم الإرسال إلى ${n} حساب`);
      showBroadcast = false; broadcastText = '';
      await load();
    } catch (err) { toast.error((err as any)?.message ?? 'تعذّر الإرسال'); }
    finally { sending = false; }
  }

  async function openLink() {
    showLink = true;
    const token = get(sessionToken);
    if (!token) return;
    try {
      const [s, t] = await Promise.all([
        peopleApi.students.list(token, 1, undefined, undefined, 500),
        peopleApi.teachers.list(token, 1)
      ]);
      students = s; teachers = t;
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر التحميل'); }
  }

  async function saveLink(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token || !linkChat.trim()) return;
    linkSaving = true;
    try {
      await telegramApi.link(token, linkChat.trim(), linkType, Number(linkPerson), linkPhone);
      toast.ok('تم ربط الحساب');
      showLink = false; linkChat = ''; linkPhone = '';
      await load();
    } catch (err) { toast.error((err as any)?.message ?? 'تعذّر الربط'); }
    finally { linkSaving = false; }
  }

  async function removeLink(id: number) {
    if (!confirm('إلغاء ربط هذا الحساب؟')) return;
    const token = get(sessionToken);
    if (!token) return;
    try { await telegramApi.unlink(token, id); toast.ok('تم إلغاء الربط'); await load(); }
    catch (e) { toast.error((e as any)?.message ?? 'تعذّر الإلغاء'); }
  }

  async function toggleLink(l: TelegramLink) {
    const token = get(sessionToken);
    if (!token) return;
    try { await telegramApi.setActive(token, l.id, !l.active); await load(); }
    catch (e) { toast.error((e as any)?.message ?? 'تعذّر التحديث'); }
  }

  async function loadRequests() {
    const token = get(sessionToken);
    if (!token) return;
    try { serviceRequests = await serviceRequestsApi.list(token); }
    catch (e) { toast.error((e as any)?.message ?? 'تعذّر تحميل الطلبات'); }
  }

  async function respondRequest(r: ServiceRequest, approve: boolean) {
    const token = get(sessionToken);
    if (!token) return;
    const response = prompt(approve ? 'رسالة الرد على الطالب (اختياري):' : 'سبب الرفض:', approve ? 'تمت الموافقة على طلبك — نتواصل معك قريباً' : '') ?? '';
    try {
      await serviceRequestsApi.respond(token, r.id, approve, response);
      toast.ok(approve ? 'تمت الموافقة على الطلب' : 'تم رفض الطلب');
      await loadRequests();
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر الرد'); }
  }

  onMount(load);
</script>

<div class="space-y-4">
  {#if error}<Alert kind="error" onClose={() => (error = '')}>{error}</Alert>{/if}

  <div class="ec-kpi-row">
    <Card pad={false}><div class="p-4"><div class="ec-stat-label">حسابات مرتبطة</div><div class="ec-stat-value">{stats.links}</div></div></Card>
    <Card pad={false}><div class="p-4"><div class="ec-stat-label">رسائل مسجّلة</div><div class="ec-stat-value">{stats.messages}</div></div></Card>
    <Card pad={false}><div class="p-4"><div class="ec-stat-label">إشعارات مُرسلة</div><div class="ec-stat-value">{stats.notifications}</div></div></Card>
    <Card pad={false}><div class="p-4"><div class="ec-stat-label">حالة البوت</div><div class="ec-stat-value" style="color:{cfg.tg_enabled === '1' ? 'var(--ec-teal)' : 'var(--ec-text-3)'}">{cfg.tg_enabled === '1' ? 'مفعّل' : 'متوقف'}</div></div></Card>
  </div>

  <div class="ec-grid ec-cols-2">
    <Card title="إعدادات البوت">
      <div class="space-y-4">
        <Field label="رمز البوت (Bot Token)" hint="احصل عليه من @BotFather على Telegram — يُحفظ في قاعدة البيانات.">
          <input class="ec-input" type="password" dir="ltr" bind:value={cfg.tg_token} placeholder="123456:ABC-DEF..." />
        </Field>
        <Field label="معرّفات المديرين" hint="معرّفات محادثات Telegram للمديرين، مفصولة بفواصل — لأوامر الإذاعة والتنبيهات.">
          <input class="ec-input" dir="ltr" bind:value={cfg.tg_manager_chats} placeholder="123456789,987654321" />
        </Field>
        <div class="space-y-2">
          {#each TOGGLES as t (t.key)}
            <label class="flex items-start gap-3 rounded-lg px-3 py-2" style="background:var(--ec-surface-2)">
              <input type="checkbox" class="mt-1" checked={cfg[t.key] === '1' || cfg[t.key] === undefined && (t.key !== 'tg_enabled')} onchange={(e) => (cfg[t.key] = e.currentTarget.checked ? '1' : '0')} />
              <span>
                <span class="block text-[13px] font-semibold">{t.label}</span>
                <span class="block text-[12px]" style="color:var(--ec-text-3)">{t.hint}</span>
              </span>
            </label>
          {/each}
        </div>
        {#if testResult}<Alert kind={testResult.startsWith('البوت يعمل') ? 'ok' : 'error'}>{testResult}</Alert>{/if}
        <div class="flex flex-wrap gap-2">
          <Button variant="primary" loading={saving} onclick={save}>حفظ الإعدادات</Button>
          <Button variant="soft" loading={testing} onclick={test}>🔍 اختبار الاتصال</Button>
        </div>
      </div>
    </Card>

    <Card title="دليل سريع">
      <div class="space-y-2 text-[13px]" style="color:var(--ec-text-2)">
        <p>1. أنشئ بوتاً عبر <b>@BotFather</b> واحصل على الرمز.</p>
        <p>2. الصق الرمز أعلاه واضغط «حفظ» ثم «اختبار الاتصال».</p>
        <p>3. فعّل البوت، واطلب من الطلاب والمدرسين إرسال <b>/start</b> للبوت ومشاركة رقم هاتفهم المسجّل.</p>
        <p>4. تُطابق آخر 9 أرقام تلقائياً مع بيانات الطلاب/المدرسين.</p>
        <p>5. الأوامر: <b>/today /schedule /fees /attendance /courses /help</b></p>
        <p>6. استخدم «ربط يدوي» و«إذاعة» و«سجل الرسائل» أدناه.</p>
      </div>
    </Card>
  </div>

  <Card title="الحسابات المرتبطة" pad={false}>
    <div class="ec-toolbar px-5 py-4" style="margin-bottom:0">
      <Button variant="primary" size="sm" onclick={openLink}>+ ربط يدوي</Button>
      <Button variant="soft" size="sm" onclick={() => (showBroadcast = true)}>📢 إذاعة</Button>
      <Button variant="ghost" size="sm" onclick={openLog}>💬 سجل الرسائل</Button>
      <Button variant="soft" size="sm" onclick={() => { showRequests = true; loadRequests(); }}>
        📢 طلبات الخدمة {serviceRequests.filter(r => r.status === 'pending').length > 0 ? `(${serviceRequests.filter(r => r.status === 'pending').length})` : ''}
      </Button>
    </div>
    <div class="p-4">
      <DataTable columns={linkCols} rows={links} rowKey={(l) => l.id} emptyTitle="لا توجد حسابات مرتبطة" emptyHint="اطلب من الطلاب والمدرسين إرسال /start للبوت." emptyIcon="🔗">
        {#snippet actions(l: TelegramLink)}
          <Button variant="ghost" size="sm" onclick={() => toggleLink(l)}>{l.active ? 'تعطيل' : 'تفعيل'}</Button>
          <Button variant="ghost" size="sm" onclick={() => removeLink(l.id)}>إلغاء</Button>
        {/snippet}
      </DataTable>
    </div>
  </Card>
</div>

<Modal open={showBroadcast} title="إذاعة رسالة" subtitle="تُرسل لكل الحسابات المرتبطة أو لفئة محددة." onClose={() => (showBroadcast = false)}>
  <form class="space-y-4" onsubmit={doBroadcast}>
    <Field label="الفئة">
      <select class="ec-select" bind:value={broadcastTarget}>
        <option value="all">الكل</option>
        <option value="student">الطلاب</option>
        <option value="teachers">المدرسون</option>
        <option value="manager">الإدارة</option>
      </select>
    </Field>
    <Field label="نص الرسالة" required>
      <textarea class="ec-textarea" rows="5" bind:value={broadcastText} required></textarea>
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showBroadcast = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={sending}>إرسال</Button>
    </div>
  </form>
</Modal>

<Modal open={showLink} title="ربط حساب يدوي" onClose={() => (showLink = false)}>
  <form class="space-y-4" onsubmit={saveLink}>
    <Field label="معرّف المحادثة (Chat ID)" required hint="يمكن الحصول عليه من البوت بعد إرسال /start.">
      <input class="ec-input" dir="ltr" bind:value={linkChat} required placeholder="123456789" />
    </Field>
    <div class="ec-form-grid">
      <Field label="النوع">
        <select class="ec-select" bind:value={linkType} onchange={() => (linkPerson = 0)}>
          <option value="student">طالب</option>
          <option value="teacher">مدرس</option>
          <option value="manager">إدارة (معرّف مستخدم)</option>
        </select>
      </Field>
      <Field label="الهاتف"><input class="ec-input" dir="ltr" bind:value={linkPhone} /></Field>
    </div>
    <Field label="الشخص" required>
      <select class="ec-select" bind:value={linkPerson}>
        <option value={0}>— اختر —</option>
        {#if linkType === 'student'}
          {#each students as s (s.id)}<option value={s.id}>{s.full_name} (#{s.student_no})</option>{/each}
        {:else if linkType === 'teacher'}
          {#each teachers as t (t.id)}<option value={t.id}>{t.full_name}</option>{/each}
        {:else}
          {#each links.filter((l) => l.person_type === 'manager') as l (l.id)}<option value={l.person_id}>{l.person_name}</option>{/each}
        {/if}
      </select>
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (showLink = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={linkSaving} disabled={!linkPerson}>ربط</Button>
    </div>
  </form>
</Modal>

<Modal open={showLog} title="سجل الرسائل" onClose={() => (showLog = false)} width="1000px">
  <div class="ec-toolbar mb-3" style="margin-bottom:0">
    <input class="ec-input grow" type="search" placeholder="بحث في الرسائل…" bind:value={logSearch} oninput={loadMessages} />
    <select class="ec-select" style="max-width:160px" bind:value={logKind} onchange={loadMessages}>
      <option value="">كل الأنواع</option>
      <option value="command">أوامر</option>
      <option value="contact">ربط</option>
      <option value="payment">دفع</option>
      <option value="attendance">حضور</option>
      <option value="reminder">تذكير</option>
      <option value="broadcast">إذاعة</option>
    </select>
    <Button variant="ghost" size="sm" onclick={loadMessages}>↻ تحديث</Button>
  </div>
  <DataTable columns={msgCols} rows={messages} rowKey={(m) => m.id} emptyTitle="لا توجد رسائل" emptyIcon="💬" />
</Modal>

<Modal open={showRequests} title="طلبات الخدمة" subtitle="طلبات من أرقام غير مسجّلة عبر بوت Telegram" onClose={() => (showRequests = false)} width="820px">
  <DataTable
    columns={[
      { key: 'requester', label: 'الاسم' },
      { key: 'phone', label: 'الهاتف', num: true },
      { key: 'service', label: 'الخدمة' },
      { key: 'details', label: 'التفاصيل' },
      { key: 'status', label: 'الحالة' },
      { key: 'created_at', label: 'الوقت', format: (v) => String(v).slice(0, 16).replace('T', ' ') }
    ]}
    rows={serviceRequests}
    rowKey={(r) => r.id}
    emptyTitle="لا توجد طلبات"
    emptyIcon="📢"
  >
    {#snippet actions(r: ServiceRequest)}
      {#if r.status === 'pending'}
        <Button variant="success" size="sm" onclick={() => respondRequest(r, true)}>موافقة</Button>
        <Button variant="danger" size="sm" onclick={() => respondRequest(r, false)}>رفض</Button>
      {:else}
        <Badge status={r.status} />
      {/if}
    {/snippet}
  </DataTable>
</Modal>
