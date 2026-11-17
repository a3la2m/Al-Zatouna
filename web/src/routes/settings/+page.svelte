<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken, sessionUser } from '$lib/stores/app';
  import { goto } from '$app/navigation';
  import {
    usersApi, settingsApi, centerApi, accountApi, backupApi,
    type AppInfo, type UserSummary, type CenterSettings, type AccountTelegramStatus
  } from '$lib/api/app';
  import { toast } from '$lib/toasts';
  import { invoke } from '$lib/api';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Card from '$lib/components/Card.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Field from '$lib/components/Field.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import TelegramSettings from '$lib/components/TelegramSettings.svelte';
  import DataTable from '$lib/components/DataTable.svelte';
  import { ago, count, date } from '$lib/format';

  let tab = $state('users');

  let users = $state<UserSummary[]>([]);
  let roles = $state<string[]>([]);
  let info = $state<AppInfo | null>(null);
  let loading = $state(true);
  let error = $state('');

  // add-user form
  let showAdd = $state(false);
  let username = $state('');
  let fullName = $state('');
  let password = $state('');
  let pickedRoles = $state<string[]>([]);
  let saving = $state(false);
  let formError = $state('');

  // reset-password form
  let resetTarget = $state<UserSummary | null>(null);
  let resetPassword = $state('');
  let resetReason = $state('');
  let resetting = $state(false);

  // data directory
  // center + print settings
  let center = $state<CenterSettings | null>(null);
  let centerForm = $state<Record<string, string>>({});
  let savingCenter = $state(false);
  let logoFile = $state<File | null>(null);

  let newPath = $state('');
  let changingDir = $state(false);

  // own password
  let curPw = $state('');
  let newPw = $state('');
  let savingPw = $state(false);
  let recoveryCodes = $state<string[]>([]);
  let generatingCodes = $state(false);

  // my phone + telegram linking
  let tgAccount = $state<AccountTelegramStatus | null>(null);
  let phoneInput = $state('');
  let savingPhone = $state(false);
  let loadingAccountState = $state(false);
  let linkCode = $state('');
  let makingCode = $state(false);
  let unlinkingTg = $state(false);
  let copiedCode = $state(false);

  // backup / share / import
  let exporting = $state(false);
  let exportedPath = $state('');
  let importing = $state(false);

  const TABS = [
    { id: 'users', label: 'المستخدمون' },
    { id: 'center', label: 'المركز والطباعة' },
    { id: 'telegram', label: 'التلجرام' },
    { id: 'data', label: 'ملفات ومجلد البيانات' },
    { id: 'account', label: 'حسابي' },
    { id: 'about', label: 'عن التطبيق' }
  ];

  async function load() {
    const token = get(sessionToken);
    if (!token) {
      loading = false;
      return;
    }
    loading = true;
    error = '';
    try {
      const [u, r, i, c] = await Promise.all([
        usersApi.list(token),
        usersApi.roles(token),
        settingsApi.info(),
        centerApi.get()
      ]);
      users = u;
      roles = r;
      info = i;
      center = c;
      centerForm = {
        name: c.name, trade_name: c.trade_name, address: c.address, phones: c.phones,
        email: c.email, website: c.website, license_no: c.license_no, tax_no: c.tax_no,
        academic_year: c.academic_year, header_text: c.header_text, footer_text: c.footer_text,
        logo_ref: c.logo_ref
      };
      newPath = i.data_dir;
    } catch (e) {
      error = (e as Error)?.message ?? 'تعذّر تحميل الإعدادات';
    } finally {
      loading = false;
    }
  }

  async function createUser(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    formError = '';
    saving = true;
    try {
      await usersApi.create(token, {
        username: username.trim(),
        full_name: fullName.trim(),
        password,
        roles: pickedRoles
      });
      toast.ok('تم إنشاء المستخدم');
      showAdd = false;
      username = '';
      fullName = '';
      password = '';
      pickedRoles = [];
      await load();
    } catch (err) {
      formError = (err as Error)?.message ?? 'تعذّر إنشاء المستخدم';
    } finally {
      saving = false;
    }
  }

  async function toggleUser(u: UserSummary) {
    const token = get(sessionToken);
    if (!token) return;
    try {
      if (u.is_active) {
        const reason = prompt('سبب التعطيل؟', 'لم يعد مطلوباً') ?? '';
        if (!reason) return;
        await usersApi.disable(token, u.id, reason);
        toast.ok(`تم تعطيل ${u.username}`);
      } else {
        await usersApi.enable(token, u.id);
        toast.ok(`تم تفعيل ${u.username}`);
      }
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر التنفيذ');
    }
  }

  async function doReset(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token || !resetTarget) return;
    resetting = true;
    try {
      await usersApi.resetPassword(
        token,
        resetTarget.id,
        resetPassword,
        resetReason || 'إعادة تعيين من الإعدادات'
      );
      toast.ok(`تمت إعادة تعيين كلمة مرور ${resetTarget.username}`);
      resetTarget = null;
      resetPassword = '';
      resetReason = '';
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر إعادة التعيين');
    } finally {
      resetting = false;
    }
  }

  async function changeDir(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token || !newPath.trim()) return;
    if (
      !confirm(
        'سيتم نسخ قاعدة البيانات الحالية إلى المجلد الجديد والانتقال إليه.\nهل تريد المتابعة؟'
      )
    )
      return;
    changingDir = true;
    try {
      const res = await settingsApi.changeDataDir(token, newPath.trim());
      toast.ok(res.changed ? 'تم نقل مجلد البيانات بنجاح' : 'المجلد لم يتغيّر');
      await load();
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر تغيير مجلد البيانات');
    } finally {
      changingDir = false;
    }
  }

  async function changeMyPassword(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    savingPw = true;
    try {
      const { authApi } = await import('$lib/api/auth');
      await authApi.changePassword(token, curPw, newPw);
      toast.ok('تم تغيير كلمة المرور');
      curPw = '';
      newPw = '';
    } catch (err) {
      toast.error((err as Error)?.message ?? 'تعذّر تغيير كلمة المرور');
    } finally {
      savingPw = false;
    }
  }

  async function saveCenter(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    savingCenter = true;
    try {
      if (logoFile) {
        const b64 = await new Promise<string>((resolve, reject) => {
          const fr = new FileReader();
          fr.onload = () => resolve(String(fr.result).split(',')[1] ?? '');
          fr.onerror = () => reject(new Error('تعذّر قراءة الصورة'));
          fr.readAsDataURL(logoFile!);
        });
        const name = await centerApi.uploadLogo(token, logoFile.name, b64);
        centerForm.logo_ref = name;
      }
      center = await centerApi.update(token, centerForm);
      toast.ok('تم حفظ إعدادات المركز والطباعة');
      logoFile = null;
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر حفظ الإعدادات');
    } finally {
      savingCenter = false;
    }
  }

  async function generateMyCodes() {
    const token = get(sessionToken);
    if (!token || !$sessionUser) return;
    generatingCodes = true;
    try {
      recoveryCodes = await invoke<string[]>('auth_generate_recovery_codes', {
        token, user_id: $sessionUser.user_id, count: 6
      });
      toast.ok('تم إنشاء رموز الاسترداد — احتفظ بها في مكان آمن');
    } catch (e) { toast.error((e as any)?.message ?? 'تعذّر إنشاء الرموز'); }
    finally { generatingCodes = false; }
  }

  async function loadAccount() {
    const token = get(sessionToken);
    if (!token) return;
    loadingAccountState = true;
    try {
      tgAccount = await accountApi.telegramStatus(token);
      phoneInput = tgAccount.phone;
    } catch (e) {
      toast.error((e as any)?.message ?? 'تعذّر تحميل حالة الربط');
    } finally {
      loadingAccountState = false;
    }
  }

  async function savePhone(e: SubmitEvent) {
    e.preventDefault();
    const token = get(sessionToken);
    if (!token) return;
    savingPhone = true;
    try {
      const saved = await accountApi.setPhone(token, phoneInput.trim());
      phoneInput = saved;
      toast.ok(saved ? 'تم حفظ رقم الجوال' : 'تم حذف رقم الجوال');
      await loadAccount();
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر حفظ رقم الجوال');
    } finally {
      savingPhone = false;
    }
  }

  async function makeLinkCode() {
    const token = get(sessionToken);
    if (!token) return;
    makingCode = true;
    copiedCode = false;
    try {
      linkCode = await accountApi.createLinkCode(token);
      toast.ok('أُنشئ رمز ربط صالح لمدة 10 دقائق');
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر إنشاء رمز الربط');
    } finally {
      makingCode = false;
    }
  }

  async function copyLinkCode() {
    if (!linkCode) return;
    try {
      await navigator.clipboard.writeText(linkCode);
      copiedCode = true;
      setTimeout(() => (copiedCode = false), 2000);
    } catch {
      toast.error('تعذّر النسخ — انسخ الرمز يدوياً');
    }
  }

  async function openBotChat() {
    const token = get(sessionToken);
    if (!token || !tgAccount?.bot_username) return;
    const url = linkCode
      ? `https://t.me/${tgAccount.bot_username}?start=${encodeURIComponent(linkCode)}`
      : `https://t.me/${tgAccount.bot_username}`;
    try {
      await accountApi.openUrl(token, url);
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر فتح تلجرام');
    }
  }

  async function unlinkTelegram() {
    const token = get(sessionToken);
    if (!token) return;
    if (!confirm('سيتم فصل حسابك عن بوت التلجرام. هل تريد المتابعة؟')) return;
    unlinkingTg = true;
    try {
      await accountApi.unlinkTelegram(token);
      linkCode = '';
      toast.ok('تم فصل الحساب عن البوت');
      await loadAccount();
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر فصل الحساب');
    } finally {
      unlinkingTg = false;
    }
  }

  async function exportArchive() {
    const token = get(sessionToken);
    if (!token) return;
    exporting = true;
    try {
      exportedPath = await backupApi.exportArchive(token);
      toast.ok('تم إنشاء الأرشيف المضغوط');
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر إنشاء الأرشيف');
    } finally {
      exporting = false;
    }
  }

  async function openExportedFolder() {
    const token = get(sessionToken);
    if (!token || !exportedPath) return;
    const dir = exportedPath.slice(0, exportedPath.lastIndexOf('/')) || exportedPath;
    try {
      await backupApi.openPath(token, dir);
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر فتح المجلد');
    }
  }

  async function importArchive() {
    const token = get(sessionToken);
    if (!token) return;
    if (
      !confirm(
        'سيتم فتح قاعدة البيانات المختارة واستبدال الجلسة الحالية.\nسيُطلب منك تسجيل الدخول من جديد. هل تريد المتابعة؟'
      )
    )
      return;
    importing = true;
    try {
      await backupApi.importArchive(token);
      toast.ok('تم استيراد قاعدة البيانات — يُرجى تسجيل الدخول');
      sessionToken.set(null);
      sessionUser.set(null);
      await goto('/');
    } catch (err) {
      toast.error((err as any)?.message ?? 'تعذّر الاستيراد');
    } finally {
      importing = false;
    }
  }

  $effect(() => {
    if (tab === 'account' && !tgAccount) loadAccount();
  });

  onMount(load);
</script>

{#snippet rolesCell(u: UserSummary)}
  <div class="flex flex-wrap gap-1">
    {#each (u.roles.length ? u.roles : ['—']) as r (r)}
      <Badge label={r} tone="info" dot={false} />
    {/each}
  </div>
{/snippet}

{#snippet statusCell(u: UserSummary)}
  <Badge status={u.is_active ? 'active' : 'inactive'} />
{/snippet}

<PageHeader title="الإعدادات" subtitle="المستخدمون، ملفات البيانات، حسابك ومعلومات التطبيق">
  <Button variant="ghost" size="sm" onclick={load} loading={loading}>↻ تحديث</Button>
</PageHeader>

{#if error}
  <div class="mb-4">
    <Alert kind="error" onClose={() => (error = '')}>{error}</Alert>
  </div>
{/if}

<div class="mb-5"><Tabs items={TABS} bind:value={tab} /></div>

{#if loading && !info && users.length === 0}
  <div class="ec-skel h-64 rounded-2xl"></div>
{:else if tab === 'telegram'}
  <TelegramSettings />
{:else if tab === 'users'}
  <Card pad={false}>
    <div
      class="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4"
      style="border-color:var(--ec-border)"
    >
      <div>
        <h2 class="ec-card-title">إدارة المستخدمين</h2>
        <p class="mt-0.5 text-[12.5px]" style="color:var(--ec-text-3)">
          {count(users.length)} مستخدم — الحذف غير متاح، التعطيل يحفظ السجل
        </p>
      </div>
      <Button variant="primary" size="sm" onclick={() => (showAdd = true)}>
        + إضافة مستخدم
      </Button>
    </div>

    <div class="p-4">
      <DataTable
        columns={[
          { key: 'username', label: 'اسم المستخدم' },
          { key: 'full_name', label: 'الاسم الكامل' },
          { key: 'roles', label: 'الأدوار', render: rolesCell },
          { key: 'is_active', label: 'الحالة', render: statusCell },
          {
            key: 'last_login_at',
            label: 'آخر دخول',
            format: (v) => (v ? ago(String(v)) : 'لم يسجّل دخولاً')
          },
          { key: 'created_at', label: 'أُنشئ', format: (v) => date(String(v)) }
        ]}
        rows={users}
        rowKey={(u) => u.id}
        emptyTitle="لا يوجد مستخدمون"
        emptyHint="أنشئ أول مستخدم من زر «إضافة مستخدم»."
      >
        {#snippet actions(u: UserSummary)}
          <Button
            variant="ghost"
            size="sm"
            onclick={() => {
              resetTarget = u;
              resetPassword = '';
              resetReason = '';
            }}
          >
            إعادة تعيين
          </Button>
          <Button
            variant={u.is_active ? 'danger' : 'success'}
            size="sm"
            onclick={() => toggleUser(u)}
            disabled={u.id === $sessionUser?.user_id}
          >
            {u.is_active ? 'تعطيل' : 'تفعيل'}
          </Button>
        {/snippet}
      </DataTable>
    </div>
  </Card>
{:else if tab === 'data'}
  <div class="ec-grid ec-cols-2">
    <Card title="مجلد البيانات الحالي">
      <div class="space-y-3 text-sm">
        <div>
          <div class="ec-label mb-1">المسار</div>
          <div
            class="ec-mono rounded-lg px-3 py-2.5 text-[13px]"
            style="background:var(--ec-surface-2); border:1px solid var(--ec-border); word-break:break-all"
            dir="ltr"
          >
            {info?.data_dir ?? '—'}
          </div>
        </div>
        <ul class="space-y-1.5 text-[13px]" style="color:var(--ec-text-2)">
          <li>• <span class="ec-mono">database/education_center.db</span> — قاعدة البيانات</li>
          <li>• <span class="ec-mono">exports/</span> — ملفات Excel المصدَّرة</li>
          <li>• <span class="ec-mono">backups/</span> — نسخ الأحتياطي</li>
          <li>• الإعدادات محفوظة في ملف <span class="ec-mono">settings.json</span></li>
        </ul>
        <Alert kind="info">
          التطبيق يعمل بالكامل على جهازك — لا يوجد خادم خارجي ولا إنترنت مطلوب.
        </Alert>
      </div>
    </Card>

    <Card title="نقل إلى مجلد آخر">
      <form class="space-y-3" onsubmit={changeDir}>
        <Field
          label="المسار الجديد"
          hint="سيتم نسخ قاعدة البيانات الحالية إلى المجلد الجديد ثم المتابعة منه."
        >
          <input
            class="ec-input"
            dir="ltr"
            bind:value={newPath}
            placeholder="/home/alaa/Documents/ec-data"
            required
          />
        </Field>
        <div class="flex gap-2">
          <Button type="submit" variant="primary" loading={changingDir}>
            نقل مجلد البيانات
          </Button>
          <Button variant="ghost" onclick={() => (newPath = info?.data_dir ?? '')}>
            إعادة التعيين
          </Button>
        </div>
      </form>

      <div class="ec-divider"></div>

      <div class="text-[13px]" style="color:var(--ec-text-2)">
        <div class="ec-label mb-2">أمثلة على المسارات</div>
        <div class="flex flex-wrap gap-2">
          <code
            class="ec-mono rounded-md px-2 py-1 text-[12px]"
            style="background:var(--ec-surface-2)"
          >
            ~/Documents/education-center
          </code>
          <code
            class="ec-mono rounded-md px-2 py-1 text-[12px]"
            style="background:var(--ec-surface-2)"
          >
            /mnt/usb/ec-data
          </code>
        </div>
      </div>
    </Card>
  </div>

  <div class="mt-4">
    <Card title="مشاركة ونسخ احتياطي">
      <div class="space-y-4">
        <div>
          <p class="text-[13px]" style="color:var(--ec-text-2)">
            أنشئ أرشيفاً مضغوطاً يحتوي على قاعدة البيانات (لقطة متسقة) وملف الإعدادات
            وشعار المركز — لمشاركته أو حفظه على قرص خارجي.
          </p>
          <div class="mt-3 flex flex-wrap items-center gap-2">
            <Button variant="primary" size="sm" loading={exporting} onclick={exportArchive}>
              💾 حفظ أرشيف مضغوط…
            </Button>
            {#if exportedPath}
              <Button variant="ghost" size="sm" onclick={openExportedFolder}>📂 فتح المجلد</Button>
            {/if}
          </div>
          {#if exportedPath}
            <div
              class="ec-mono mt-3 rounded-lg px-3 py-2 text-[12px]"
              style="background:var(--ec-surface-2); border:1px solid var(--ec-border); word-break:break-all"
              dir="ltr"
            >
              {exportedPath}
            </div>
          {/if}
        </div>

        <div class="ec-divider"></div>

        <div>
          <p class="text-[13px]" style="color:var(--ec-text-2)">
            افتح قاعدة بيانات أخرى من أرشيف <span class="ec-mono">.zip</span> أو ملف
            <span class="ec-mono">.db</span>. تُحفظ نسخة من قاعدة البيانات الحالية قبل التبديل
            (داخل مجلد البيانات الحالي)، ولن تُفقد بياناتك.
          </p>
          <div class="mt-3">
            <Button variant="soft" size="sm" loading={importing} onclick={importArchive}>
              📥 استيراد وفتح قاعدة بيانات…
            </Button>
          </div>
        </div>
      </div>
    </Card>
  </div>
{:else if tab === 'center'}
  <div class="ec-grid ec-cols-2">
    <Card title="هوية المركز">
      <form class="space-y-4" onsubmit={saveCenter}>
        <Field label="اسم المركز" required>
          <input class="ec-input" bind:value={centerForm.name} required />
        </Field>
        <Field label="الاسم التجاري">
          <input class="ec-input" bind:value={centerForm.trade_name} />
        </Field>
        <div class="ec-form-grid">
          <Field label="رقم الرخصة"><input class="ec-input" bind:value={centerForm.license_no} /></Field>
          <Field label="الرقم الضريبي"><input class="ec-input" bind:value={centerForm.tax_no} /></Field>
        </div>
        <Field label="العنوان"><input class="ec-input" bind:value={centerForm.address} /></Field>
        <div class="ec-form-grid">
          <Field label="الهواتف"><input class="ec-input" dir="ltr" bind:value={centerForm.phones} /></Field>
          <Field label="البريد الإلكتروني"><input class="ec-input" type="email" dir="ltr" bind:value={centerForm.email} /></Field>
        </div>
        <div class="ec-form-grid">
          <Field label="الموقع"><input class="ec-input" dir="ltr" bind:value={centerForm.website} /></Field>
          <Field label="السنة الأكاديمية"><input class="ec-input" bind:value={centerForm.academic_year} /></Field>
        </div>

        <div class="ec-divider"></div>
        <h3 class="ec-card-title">الطباعة</h3>
        <Field label="شعار المركز" hint="PNG أو JPG — يُحفظ في مجلد البيانات ويُضمَّن في كل مستند.">
          <input
            class="ec-input"
            type="file"
            accept="image/png,image/jpeg"
            onchange={(e) => (logoFile = e.currentTarget.files?.[0] ?? null)}
          />
        </Field>
        {#if center?.logo_ref}
          <div class="flex items-center gap-2 text-[13px]" style="color:var(--ec-text-2)">
            الشعار الحالي: <code class="ec-badge ec-badge-info no-dot">{center.logo_ref}</code>
          </div>
        {/if}
        <Field label="نص رأس الصفحة" hint="يظهر أعلى المستند بعد اسم المركز.">
          <textarea class="ec-textarea" rows="2" bind:value={centerForm.header_text}></textarea>
        </Field>
        <Field label="نص تذييل الصفحة" hint="يظهر أسفل كل مستند.">
          <textarea class="ec-textarea" rows="2" bind:value={centerForm.footer_text}></textarea>
        </Field>

        <div class="flex justify-end">
          <Button type="submit" variant="primary" loading={savingCenter}>حفظ الإعدادات</Button>
        </div>
      </form>
    </Card>

    <Card title="معاينة الهوية">
      <div class="rounded-xl border p-6 text-center" style="border-color:var(--ec-border); background:var(--ec-surface-2)">
        <div class="mx-auto mb-3 flex h-20 w-20 items-center justify-center rounded-xl" style="background:var(--ec-surface); border:1px solid var(--ec-border)">
          {#if center?.logo_ref}
            <span class="text-3xl">🖼</span>
          {:else}
            <span class="text-3xl" style="color:var(--ec-text-3)">🏛</span>
          {/if}
        </div>
        <h2 class="text-xl font-bold">{center?.name || '—'}</h2>
        {#if center?.trade_name}<p class="text-sm" style="color:var(--ec-text-2)">{center.trade_name}</p>{/if}
        <div class="mt-3 text-[13px]" style="color:var(--ec-text-2)">
          <p>{center?.address}</p>
          <p>{center?.phones} {center?.email}</p>
        </div>
        {#if center?.header_text}
          <div class="mt-3 border-t pt-3 text-[12px]" style="border-color:var(--ec-border)">{center.header_text}</div>
        {/if}
        {#if center?.footer_text}
          <div class="mt-3 border-t pt-3 text-[12px]" style="border-color:var(--ec-border)">{center.footer_text}</div>
        {/if}
      </div>
      <Alert kind="info">تُطبَّق هذه الإعدادات فوراً على سندات القبض والصرف وإيصالات التسجيل عند الطباعة.</Alert>
    </Card>
  </div>
{:else if tab === 'account'}
  <div class="mb-4">
    <Card title="رقم الجوال وربط التلجرام">
      <div class="space-y-4">
        <div class="flex flex-wrap items-center gap-2.5">
          <span class="text-[13px] font-semibold" style="color:var(--ec-text-2)">حالة الربط:</span>
          {#if tgAccount?.linked}
            <Badge
              label={tgAccount.link_active ? 'مربوط بالبوت' : 'مربوط — موقوف'}
              tone={tgAccount.link_active ? 'ok' : 'warn'}
            />
          {:else}
            <Badge label="غير مربوط" tone="muted" />
          {/if}
          {#if tgAccount && !tgAccount.bot_enabled}
            <Badge label="البوت غير مفعّل" tone="danger" />
          {/if}
          <Button variant="ghost" size="sm" loading={loadingAccountState} onclick={loadAccount}>
            ↻ تحديث الحالة
          </Button>
        </div>

        <form class="space-y-3" onsubmit={savePhone}>
          <Field label="رقم الجوال" hint="يُحفظ في ملفك ويُستخدم للربط والتواصل.">
            <input
              class="ec-input"
              dir="ltr"
              inputmode="tel"
              bind:value={phoneInput}
              placeholder="05xxxxxxxx"
            />
          </Field>
          <div class="flex justify-end">
            <Button type="submit" variant="primary" loading={savingPhone}>حفظ الرقم</Button>
          </div>
        </form>

        {#if tgAccount?.linked}
          <div class="rounded-xl border p-3 text-[12.5px]" style="border-color:var(--ec-border); background:var(--ec-surface-2)">
            <div class="flex flex-wrap gap-x-6 gap-y-1">
              <span>معرّف المحادثة: <code class="ec-mono" dir="ltr">{tgAccount.chat_id ?? '—'}</code></span>
              <span>تاريخ الربط: {tgAccount.linked_at ? ago(tgAccount.linked_at) : '—'}</span>
            </div>
          </div>
          <div class="flex flex-wrap gap-2">
            <Button variant="ghost" size="sm" loading={makingCode} onclick={makeLinkCode}>
              🔑 رمز ربط جديد
            </Button>
            <Button variant="danger" size="sm" loading={unlinkingTg} onclick={unlinkTelegram}>
              فصل الحساب عن البوت
            </Button>
          </div>
        {:else}
          <div class="flex flex-wrap gap-2">
            <Button variant="soft" size="sm" loading={makingCode} onclick={makeLinkCode}>
              🔑 إنشاء رمز ربط
            </Button>
          </div>
        {/if}

        {#if linkCode}
          <div class="rounded-xl border p-4" style="border-color:var(--ec-border); background:var(--ec-surface-2)">
            <p class="mb-2 text-[13px]" style="color:var(--ec-text-2)">
              افتح بوت التلجرام وأرسل الرمز التالي (صالح 10 دقائق، لمرة واحدة):
            </p>
            <div class="flex flex-wrap items-center gap-2">
              <code class="rounded-lg px-3 py-2 text-[16px] font-bold tracking-wider" dir="ltr" style="background:var(--ec-surface); border:1px solid var(--ec-border)">
                {linkCode}
              </code>
              <Button variant="ghost" size="sm" onclick={copyLinkCode}>
                {copiedCode ? '✓ تم النسخ' : 'نسخ الرمز'}
              </Button>
              {#if tgAccount?.bot_username}
                <Button variant="soft" size="sm" onclick={openBotChat}>
                  ✈️ فتح البوت وطلب الربط
                </Button>
              {/if}
            </div>
            <p class="mt-2 text-[12px]" style="color:var(--ec-text-3)">
              بدلاً من ذلك أرسل في البوت: <code class="ec-mono" dir="ltr">/link {linkCode}</code>
            </p>
          </div>
        {/if}

        {#if !tgAccount?.bot_username}
          <Alert kind="info">
            فعّل بوت التلجرام من تبويب «التلجرام» أولاً، ثم اضغط «تحديث الحالة» ليظهر زر فتح البوت.
          </Alert>
        {/if}
      </div>
    </Card>
  </div>

  <div class="ec-grid ec-cols-2">
    <Card title="بيانات الجلسة">
      <dl class="space-y-2.5 text-sm">
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">الاسم الكامل</dt>
          <dd class="font-semibold">{$sessionUser?.full_name}</dd>
        </div>
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">اسم المستخدم</dt>
          <dd class="ec-mono font-semibold">{$sessionUser?.username}</dd>
        </div>
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">عدد الصلاحيات</dt>
          <dd class="font-semibold">{count($sessionUser?.permissions.length ?? 0)}</dd>
        </div>
      </dl>
    </Card>

    <Card title="تغيير كلمة المرور">
      <form class="space-y-3" onsubmit={changeMyPassword}>
        <Field label="كلمة المرور الحالية" required>
          <input
            class="ec-input"
            type="password"
            bind:value={curPw}
            required
            autocomplete="current-password"
          />
        </Field>
        <Field label="كلمة المرور الجديدة" hint="12 حرفاً على الأقل، مع حروف وأرقام ورموز.">
          <input
            class="ec-input"
            type="password"
            bind:value={newPw}
            required
            autocomplete="new-password"
          />
        </Field>
        <Button type="submit" variant="primary" loading={savingPw}>حفظ كلمة المرور</Button>
      </form>
    </Card>

    <Card title="رموز استرداد كلمة المرور">
      <p class="mb-3 text-[13px]" style="color:var(--ec-text-2)">
        رموز تستخدمها لاستعادة الدخول إذا نسيت كلمة المرور — كل رمز يُستخدم مرة واحدة.
      </p>
      <Button variant="soft" size="sm" loading={generatingCodes} onclick={generateMyCodes}>
        🔑 إنشاء 6 رموز استرداد
      </Button>
      {#if recoveryCodes.length}
        <div class="mt-3 rounded-xl border p-3" style="border-color:var(--ec-border); background:var(--ec-surface-2)">
          <div class="ec-err mb-2 text-[12px] font-bold">احتفظ بهذه الرموز الآن — لن تظهر مرة أخرى.</div>
          <div class="grid gap-1.5" style="grid-template-columns:repeat(2,1fr)">
            {#each recoveryCodes as c, i (i)}
              <code class="rounded px-2 py-1 text-[12px]" style="background:var(--ec-surface)">{c}</code>
            {/each}
          </div>
        </div>
      {/if}
    </Card>
  </div>
{:else}
  <div class="ec-grid ec-cols-2">
    <Card title="عن التطبيق">
      <dl class="space-y-2.5 text-sm">
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">الاسم</dt>
          <dd class="font-semibold">{info?.app ?? '—'}</dd>
        </div>
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">الإصدار</dt>
          <dd class="ec-mono font-semibold">{info?.version ?? '—'}</dd>
        </div>
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">الحسابات المحاسبية المزروعة</dt>
          <dd class="font-semibold">{count(info?.accounts_seeded ?? 0)}</dd>
        </div>
        <div class="flex justify-between gap-3">
          <dt style="color:var(--ec-text-2)">المستخدمون</dt>
          <dd class="font-semibold">{count(info?.users ?? 0)}</dd>
        </div>
      </dl>
    </Card>

    <Card title="الخصوصية والتشغيل">
      <ul class="space-y-2 text-[13.5px]" style="color:var(--ec-text-2)">
        <li>• البيانات مخزّنة محلياً في SQLite على هذا الجهاز فقط.</li>
        <li>• لا يتصل التطبيق بأي خادم خارجي.</li>
        <li>• كلمات المرور مشفّرة بـ Argon2id.</li>
        <li>• العمليات المالية الحساسة تمر بنظام «منشئ/مراجع».</li>
        <li>• كل عملية حساسة مسجّلة في سجل التدقيق.</li>
      </ul>
    </Card>
  </div>
{/if}

<!-- ------------------------- add user ------------------------- -->
<Modal
  open={showAdd}
  title="إضافة مستخدم جديد"
  subtitle="سيُطلب من المستخدم تغيير كلمة المرور عند أول دخول."
  onClose={() => (showAdd = false)}
>
  <form class="space-y-4" onsubmit={createUser}>
    {#if formError}
      <Alert kind="error">{formError}</Alert>
    {/if}
    <div class="ec-form-grid">
      <Field label="اسم المستخدم" required hint="حروف لاتينية وأرقام و _ . - (3 أحرف فأكثر)">
        <input class="ec-input" dir="ltr" bind:value={username} required minlength="3" />
      </Field>
      <Field label="الاسم الكامل" required>
        <input class="ec-input" bind:value={fullName} required />
      </Field>
    </div>
    <Field
      label="كلمة المرور"
      required
      hint="12 حرفاً على الأقل، مع حروف كبيرة وصغيرة وأرقام ورموز."
    >
      <input class="ec-input" type="password" bind:value={password} required minlength="12" />
    </Field>

    <div class="ec-field">
      <span class="ec-label">الأدوار <span class="req">*</span></span>
      <div class="flex flex-wrap gap-2">
        {#each roles as r (r)}
          <label
            class="cursor-pointer rounded-lg border px-3 py-1.5 text-[13px] font-semibold transition-colors"
            style={pickedRoles.includes(r)
              ? 'background:var(--ec-blue-soft); border-color:var(--ec-blue); color:var(--ec-blue)'
              : 'background:var(--ec-surface-2); border-color:var(--ec-border); color:var(--ec-text-2)'}
          >
            <input
              type="checkbox"
              class="ms-1.5 align-middle"
              checked={pickedRoles.includes(r)}
              onchange={() => {
                pickedRoles = pickedRoles.includes(r)
                  ? pickedRoles.filter((x) => x !== r)
                  : [...pickedRoles, r];
              }}
            />
            {r}
          </label>
        {/each}
      </div>
      {#if pickedRoles.length === 0}
        <span class="ec-err">اختر دوراً واحداً على الأقل</span>
      {/if}
    </div>

    <div class="flex justify-end gap-2 pt-1">
      <Button variant="ghost" onclick={() => (showAdd = false)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={saving} disabled={pickedRoles.length === 0}>
        إنشاء المستخدم
      </Button>
    </div>
  </form>
</Modal>

<!-- --------------------- reset password --------------------- -->
<Modal
  open={resetTarget !== null}
  title="إعادة تعيين كلمة المرور"
  subtitle={resetTarget ? `المستخدم: ${resetTarget.username}` : ''}
  onClose={() => (resetTarget = null)}
>
  <form class="space-y-4" onsubmit={doReset}>
    <Alert kind="warn">
      سيتم فرض تغيير كلمة المرور عند أول دخول للمستخدم بعد إعادة التعيين.
    </Alert>
    <Field label="كلمة المرور الجديدة" required>
      <input class="ec-input" type="password" bind:value={resetPassword} required minlength="12" />
    </Field>
    <Field label="السبب" hint="يُسجَّل في سجل التدقيق.">
      <input class="ec-input" bind:value={resetReason} placeholder="نسيان كلمة المرور" />
    </Field>
    <div class="flex justify-end gap-2">
      <Button variant="ghost" onclick={() => (resetTarget = null)}>إلغاء</Button>
      <Button type="submit" variant="primary" loading={resetting}>إعادة التعيين</Button>
    </div>
  </form>
</Modal>
