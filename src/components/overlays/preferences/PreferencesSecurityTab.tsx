import { createSignal, createEffect, Show, untrack } from 'solid-js';
import { createLogger } from '../../../lib/logger';
import { authState, authMethods, loadAuthMethods, setRequireAllAuth } from '../../../state/auth';
import { journals, activeJournalId } from '../../../state/journals';
import {
  preferences,
  setPreferences,
  MIN_AUTO_LOCK_TIMEOUT,
  MAX_AUTO_LOCK_TIMEOUT,
  MAX_VAULT_LOCK_MINUTES,
} from '../../../state/preferences';
import * as tauri from '../../../lib/tauri';
import { mapTauriError } from '../../../lib/errors';
import { open } from '../../../lib/dialog';
import { useI18n } from '../../../i18n';
import type { TabProps } from './shared';
import { AuthMethodsList } from './AuthMethodsList';
import { AddPasswordForm } from './AddPasswordForm';
import { AddKeypairForm } from './AddKeypairForm';
import { ChangePasswordForm } from './ChangePasswordForm';
import DeadManField from '../../common/DeadManField';

const log = createLogger('Preferences');

interface PreferencesSecurityTabProps extends TabProps {
  /** Switches Preferences to the Backups tab, for the post-removal "Review backups" action. */
  onReviewBackups?: () => void;
}

export default function PreferencesSecurityTab(props: PreferencesSecurityTabProps) {
  const t = useI18n();

  const hasPasswordSlot = () => authMethods().some((m) => m.slot_type === 'password');
  const activeJournal = () => journals().find((j) => j.id === activeJournalId());
  const isAutoProtected = () => activeJournal()?.auto_protected ?? false;
  const hasMultipleNonAutoMethods = () =>
    authMethods().filter((m) => m.slot_type !== 'auto').length >= 2;

  const [autoLockTimeoutDraft, setAutoLockTimeoutDraft] = createSignal(
    String(preferences().autoLockTimeout),
  );

  // Require-all-auth toggle
  const [requireAllAuth, setRequireAllAuthLocal] = createSignal(false);
  const [requireAllAuthError, setRequireAllAuthError] = createSignal<string | null>(null);
  /** The value the user asked to switch to, while their credentials are collected. */
  const [requireAllPending, setRequireAllPending] = createSignal<boolean | null>(null);
  const [requireAllPassword, setRequireAllPassword] = createSignal('');
  const [requireAllBusy, setRequireAllBusy] = createSignal(false);
  const [journalDeadManDays, setJournalDeadManDays] = createSignal(0);

  // Reset transient state + reload auth-related data when the overlay opens.
  createEffect(() => {
    if (props.isOpen()) {
      setRequireAllAuthError(null);
      setRequireAllPending(null);
      setRequireAllPassword('');
      setAutoLockTimeoutDraft(String(untrack(() => preferences().autoLockTimeout)));

      if (authState() === 'unlocked') {
        loadAuthMethods().catch((err) => log.error('Failed to reload auth methods:', err));
        tauri
          .journalDeadManDays()
          .then(setJournalDeadManDays)
          .catch((err) => log.warn("Failed to read the dead man's switch:", err));
        tauri
          .peekAuthSlotTypes()
          .then((peek) => setRequireAllAuthLocal(peek.require_all_auth))
          .catch((err) => log.error('Failed to load require_all_auth:', err));
      }
    }
  });

  const clampAutoLockTimeout = (value: string) =>
    Math.min(MAX_AUTO_LOCK_TIMEOUT, Math.max(MIN_AUTO_LOCK_TIMEOUT, parseInt(value, 10) || 300));

  const isTimeoutBelowMinimum = () =>
    /^\d+$/.test(autoLockTimeoutDraft()) && Number(autoLockTimeoutDraft()) < MIN_AUTO_LOCK_TIMEOUT;

  const persistAutoLockTimeout = (value: string) => {
    const clamped = clampAutoLockTimeout(value);
    setAutoLockTimeoutDraft(String(clamped));
    setPreferences({ autoLockTimeout: clamped });
  };

  // Switching either way re-splits the journal key, so every credential is collected first.
  const handleToggleRequireAllAuth = (checked: boolean) => {
    setRequireAllAuthError(null);
    setRequireAllPassword('');
    setRequireAllPending(checked);
  };

  const confirmRequireAllAuth = async (e: Event) => {
    e.preventDefault();
    const target = requireAllPending();
    if (target === null) return;
    if (hasPasswordSlot() && !requireAllPassword()) {
      setRequireAllAuthError(t('prefs.security.requireAllPasswordNeeded'));
      return;
    }
    setRequireAllAuthError(null);
    const keyFilePaths: string[] = [];
    for (const method of authMethods().filter((m) => m.slot_type === 'keypair')) {
      const path = await open({
        title: t('prefs.security.requireAllSelectKeyFile', { label: method.label }),
        multiple: false,
        directory: false,
        filters: [{ name: 'Key Files', extensions: ['key', 'txt', '*'] }],
      });
      if (typeof path !== 'string') return;
      keyFilePaths.push(path);
    }
    setRequireAllBusy(true);
    try {
      await setRequireAllAuth(
        target,
        hasPasswordSlot() ? requireAllPassword() : null,
        keyFilePaths,
      );
      setRequireAllAuthLocal(target);
      setRequireAllPending(null);
    } catch (err) {
      setRequireAllAuthError(mapTauriError(err, t));
    } finally {
      setRequireAllPassword('');
      setRequireAllBusy(false);
    }
  };

  return (
    <div
      id="pref-panel-security"
      role="tabpanel"
      aria-labelledby="pref-tab-security"
      tabIndex={0}
      class="space-y-8 focus:outline-none"
    >
      {/* Authentication Methods */}
      <div>
        <h3 class="text-sm font-medium text-primary mb-3">
          {t('prefs.security.authMethodsTitle')}
        </h3>
        <p class="text-xs text-tertiary mb-4 leading-relaxed">
          {t('prefs.security.authMethodsHint')}
        </p>

        <AuthMethodsList isOpen={props.isOpen} onReviewBackups={props.onReviewBackups} />

        <Show when={!hasPasswordSlot()}>
          <AddPasswordForm isOpen={props.isOpen} />
        </Show>

        <AddKeypairForm isOpen={props.isOpen} />
      </div>

      {/* Require All Auth — hidden for auto-protected journals */}
      <Show when={!isAutoProtected()}>
        <div>
          <h3 class="text-sm font-medium text-primary mb-3">
            {t('prefs.security.requireAllAuthTitle')}
          </h3>
          <p class="text-xs text-tertiary mb-4 leading-relaxed">
            {t('prefs.security.requireAllAuthHint')}
          </p>
          <Show
            when={hasMultipleNonAutoMethods()}
            fallback={
              <p class="text-xs text-tertiary italic">
                {t('prefs.security.requireAllAuthNeedsTwo')}
              </p>
            }
          >
            <label class="flex items-center gap-3">
              <input
                type="checkbox"
                checked={requireAllPending() ?? requireAllAuth()}
                disabled={requireAllBusy()}
                onChange={(e) => handleToggleRequireAllAuth(e.currentTarget.checked)}
                class="h-4 w-4 rounded border-primary text-blue-600 focus:ring-blue-500"
              />
              <span class="text-sm text-secondary">{t('prefs.security.requireAllAuthLabel')}</span>
            </label>
            <Show when={requireAllPending() !== null}>
              <form
                class="mt-3 space-y-2 rounded-md border border-primary p-3"
                onSubmit={(e) => void confirmRequireAllAuth(e)}
                data-testid="require-all-confirm"
              >
                <p class="text-xs text-tertiary">{t('prefs.security.requireAllConfirmHint')}</p>
                <Show when={hasPasswordSlot()}>
                  <input
                    type="password"
                    class="w-full rounded-md border border-primary bg-primary px-3 py-2 text-sm text-primary"
                    value={requireAllPassword()}
                    onInput={(e) => setRequireAllPassword(e.currentTarget.value)}
                    placeholder={t('prefs.security.currentPasswordPlaceholder2')}
                    aria-label={t('prefs.security.currentPasswordPlaceholder2')}
                    autocomplete="current-password"
                    disabled={requireAllBusy()}
                  />
                </Show>
                <div class="flex gap-2">
                  <button
                    type="submit"
                    class="rounded-md interactive-primary px-3 py-1.5 text-sm font-medium disabled:opacity-50"
                    disabled={requireAllBusy()}
                  >
                    {t('prefs.security.requireAllConfirm')}
                  </button>
                  <button
                    type="button"
                    class="rounded-md border border-primary px-3 py-1.5 text-sm text-secondary hover:bg-hover"
                    disabled={requireAllBusy()}
                    onClick={() => {
                      setRequireAllPending(null);
                      setRequireAllPassword('');
                      setRequireAllAuthError(null);
                    }}
                  >
                    {t('common.cancel')}
                  </button>
                </div>
              </form>
            </Show>
            <Show when={requireAllAuthError()}>
              <p class="mt-2 text-sm text-error">{requireAllAuthError()}</p>
            </Show>
          </Show>
        </div>
      </Show>

      {/* Change Password — only shown when a password slot exists */}
      <Show when={hasPasswordSlot()}>
        <ChangePasswordForm isOpen={props.isOpen} />
      </Show>

      {/* Auto-Lock */}
      <div>
        <h3 class="text-sm font-medium text-primary mb-3">{t('prefs.security.autoLockTitle')}</h3>
        <div class="space-y-3">
          <label class="flex items-center gap-3">
            <input
              type="checkbox"
              checked={preferences().autoLockEnabled}
              onChange={(e) => setPreferences({ autoLockEnabled: e.currentTarget.checked })}
              class="h-4 w-4 rounded border-primary text-blue-600 focus:ring-blue-500"
            />
            <span class="text-sm text-secondary">{t('prefs.security.autoLockLabel')}</span>
          </label>
          <label class="flex items-center gap-3">
            <input
              type="checkbox"
              checked={preferences().autoLockOnFocusLoss}
              onChange={(e) => setPreferences({ autoLockOnFocusLoss: e.currentTarget.checked })}
              class="h-4 w-4 rounded border-primary text-blue-600 focus:ring-blue-500"
            />
            <span class="text-sm text-secondary">
              {t('prefs.security.autoLockOnFocusLossLabel')}
            </span>
          </label>
          <Show when={preferences().autoLockEnabled}>
            <div class="flex items-center gap-2 pl-7">
              <label class="text-sm text-secondary whitespace-nowrap">
                {t('prefs.security.autoLockTimeoutLabel')}
              </label>
              <input
                type="number"
                min={MIN_AUTO_LOCK_TIMEOUT}
                max={MAX_AUTO_LOCK_TIMEOUT}
                step="1"
                value={autoLockTimeoutDraft()}
                onInput={(e) => {
                  const next = e.currentTarget.value;
                  setAutoLockTimeoutDraft(next);
                  if (/^\d+$/.test(next)) {
                    const parsed = Number(next);
                    if (parsed >= MIN_AUTO_LOCK_TIMEOUT && parsed <= MAX_AUTO_LOCK_TIMEOUT) {
                      setPreferences({ autoLockTimeout: parsed });
                    }
                  }
                }}
                onBlur={(e) => persistAutoLockTimeout(e.currentTarget.value)}
                class="w-20 px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <span class="text-xs text-tertiary">{t('prefs.security.autoLockRange')}</span>
              <Show when={isTimeoutBelowMinimum()}>
                <p class="text-xs text-error" role="alert">
                  {t('prefs.security.autoLockTimeoutTooLow', { min: MIN_AUTO_LOCK_TIMEOUT })}
                </p>
              </Show>
            </div>
          </Show>
          <div class="flex items-center gap-2">
            <label class="text-sm text-secondary" for="vault-lock-minutes">
              {t('prefs.security.vaultLockLabel')}
            </label>
            <input
              id="vault-lock-minutes"
              type="number"
              min="0"
              max={MAX_VAULT_LOCK_MINUTES}
              step="1"
              value={preferences().vaultLockMinutes}
              onChange={(e) => {
                const n = Math.round(Number(e.currentTarget.value));
                const clamped = Number.isFinite(n)
                  ? Math.min(MAX_VAULT_LOCK_MINUTES, Math.max(0, n))
                  : 0;
                e.currentTarget.value = String(clamped);
                setPreferences({ vaultLockMinutes: clamped });
              }}
              class="w-20 px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary focus:outline-none focus:ring-2 focus:ring-blue-500"
              data-testid="vault-lock-minutes"
            />
            <span class="text-xs text-tertiary">{t('prefs.security.vaultLockHint')}</span>
          </div>
          <Show when={authState() === 'unlocked'}>
            <DeadManField
              id="journal-dead-man-days"
              label={t('deadMan.journalLabel')}
              days={journalDeadManDays()}
              onSave={async (days) => {
                await tauri.setJournalDeadMan(days);
                setJournalDeadManDays(days);
              }}
            />
          </Show>
        </div>
      </div>
    </div>
  );
}
