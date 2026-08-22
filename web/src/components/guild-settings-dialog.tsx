"use client";

import { RefreshCwIcon } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldLabel,
} from "@/components/ui/field";
import { describeApiError } from "@/lib/api";
import {
  useGuildConfigQuery,
  useMyPermissionsQuery,
  useUpdateGuildConfig,
} from "@/lib/query/guild";

interface Props {
  guildId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const RESTRICTED_CHECKBOX_ID = "guild-settings-restricted";

/** サーバー設定ダイアログ (旧 ServerSetting.vue 相当)。restricted モードの切り替えのみ */
export function GuildSettingsDialog({ guildId, open, onOpenChange }: Props) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      // 旧 v-dialog persistent と同じく外側クリックでは閉じない
      disablePointerDismissal
    >
      <DialogContent className="sm:max-w-lg">
        {/* Base UI の Dialog は閉じると Popup を unmount するので、開くたびに設定値から初期化される */}
        <GuildSettingsForm
          guildId={guildId}
          onClose={() => onOpenChange(false)}
        />
      </DialogContent>
    </Dialog>
  );
}

function GuildSettingsForm({
  guildId,
  onClose,
}: {
  guildId: string;
  onClose: () => void;
}) {
  const configQuery = useGuildConfigQuery(guildId);
  const permissionsQuery = useMyPermissionsQuery(guildId);
  const updateConfig = useUpdateGuildConfig(guildId);
  const [restricted, setRestricted] = useState(
    configQuery.data?.restricted ?? false,
  );
  const [error, setError] = useState<string | null>(null);

  const canManage = permissionsQuery.data?.can_manage_server ?? false;
  const reloading = permissionsQuery.isFetching;
  const saving = updateConfig.isPending;

  // Discord 側で権限を変えた後に押してもらう (旧 checkEditable)。チェック内容はそのまま残す
  const reload = async () => {
    setError(null);
    const result = await permissionsQuery.refetch();
    if (result.error) setError(describeApiError(result.error));
  };

  const save = async () => {
    setError(null);
    try {
      await updateConfig.mutateAsync(restricted);
      onClose();
    } catch (cause) {
      setError(describeApiError(cause));
    }
  };

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
      className="flex flex-col gap-5"
    >
      <DialogHeader>
        <DialogTitle>サーバー設定</DialogTitle>
        <DialogDescription>
          予定の追加・編集・削除を管理権限を持つユーザーに限定できます
        </DialogDescription>
      </DialogHeader>

      {permissionsQuery.data && !canManage && (
        <p className="rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
          サーバーの設定の変更には「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限を持っている必要があります
        </p>
      )}

      <Field orientation="horizontal" data-disabled={!canManage || undefined}>
        <Checkbox
          id={RESTRICTED_CHECKBOX_ID}
          checked={restricted}
          disabled={!canManage}
          onCheckedChange={(checked) => setRestricted(checked)}
        />
        <FieldContent>
          <FieldLabel htmlFor={RESTRICTED_CHECKBOX_ID} className="font-normal">
            予定の追加・編集・削除を「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限を持ったユーザーに限定する
          </FieldLabel>
        </FieldContent>
      </Field>

      <FieldDescription>
        Discord
        側でユーザーの権限を変更した場合は「再読込」を押してください。反映まで最大
        1 分ほどかかることがあります
      </FieldDescription>

      {error && (
        <div
          role="alert"
          className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      )}

      <DialogFooter>
        <Button
          type="button"
          variant="outline"
          disabled={reloading || saving}
          onClick={reload}
          className="sm:mr-auto"
        >
          <RefreshCwIcon
            data-icon="inline-start"
            className={reloading ? "animate-spin" : undefined}
          />
          {reloading ? "確認中…" : "再読込"}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={saving}
          onClick={onClose}
        >
          キャンセル
        </Button>
        <Button type="submit" disabled={!canManage || saving || reloading}>
          {saving ? "保存中…" : "保存"}
        </Button>
      </DialogFooter>
    </form>
  );
}
