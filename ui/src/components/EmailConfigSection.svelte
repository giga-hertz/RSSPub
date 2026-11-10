<script lang="ts">
    import { api } from "../lib/api";
    import { emailConfig, isAuthenticated, popup } from "../lib/store";
    import type { EmailConfig } from "../lib/types";

    let email_address = "";
    let to_email = "";
    let enable_auto_send = false;
    let statusMsg = "";
    let statusColor = "";
    let isSaving = false;

    function normalizeRecipientList(value: string): string {
        return value
            .split(",")
            .map((e) => e.trim())
            .filter((e) => e)
            .join(", ");
    }

    $: if ($isAuthenticated) {
        loadEmailConfig();
    }

    async function loadEmailConfig() {
        try {
            const config = await api("/email-config") as EmailConfig | null;
            if (config) {
                emailConfig.set(config);
                email_address = config.email_address || "";
                to_email = config.to_email || "";
                enable_auto_send = config.enable_auto_send || false;
            }
        } catch (e) {
            console.error("Failed to load email config", e);
        }
    }

    async function saveEmailConfig() {
        isSaving = true;
        statusMsg = "Saving...";
        statusColor = "var(--text-secondary)";

        to_email = normalizeRecipientList(to_email);

        try {
            await api("/email-config", "POST", {
                email_address,
                to_email,
                enable_auto_send,
            });

            statusMsg = "Saved successfully!";
            statusColor = "var(--accent-green)";
            setTimeout(() => (statusMsg = ""), 3000);

            loadEmailConfig();
        } catch (e: any) {
            statusMsg = "Error: " + e.message;
            statusColor = "var(--danger)";
        } finally {
            isSaving = false;
        }
    }

    async function handleToggleChange() {
        if (!enable_auto_send) {
            await saveEmailConfig();
        }
    }
</script>

<section id="email-config-section" class="card" style="grid-column: 1 / -1">
    <div class="card-header">
        <img src="/icons/mail.svg" alt="Email Icon" width="20" height="20" />
        <h2>Email Configuration (Resend HTTP)</h2>
        <label
            style="margin-left: auto; display: flex; align-items: center; gap: 10px;"
        >
            <span style="font-size: 0.9rem; color: var(--text-secondary);"
                >Enable Auto Send</span
            >
            <label class="switch">
                <input
                    type="checkbox"
                    bind:checked={enable_auto_send}
                    on:change={handleToggleChange}
                />
                <span class="slider round"></span>
            </label>
        </label>
    </div>

    {#if enable_auto_send}
        <form on:submit|preventDefault={saveEmailConfig} id="email-config-form">
            <div
                id="email-inputs-wrapper"
                style="display: flex; flex-direction: column; gap: 15px; margin-bottom: 15px;"
            >
                <div>
                    <label style="display: block; font-size: 0.85rem; color: var(--text-secondary); margin-bottom: 5px;">
                        Sender Email Address (Optional, defaults to Render environment variable)
                    </label>
                    <input
                        type="email"
                        bind:value={email_address}
                        placeholder="e.g. updates@yourdomain.com"
                    />
                </div>

                <div>
                    <label style="display: block; font-size: 0.85rem; color: var(--text-secondary); margin-bottom: 5px;">
                        Recipient Email(s)
                    </label>
                    <input
                        type="email"
                        bind:value={to_email}
                        placeholder="kindle@example.com, backup@example.com"
                        multiple
                        required
                    />
                </div>
            </div>
            <div style="margin: 0 0 15px; font-size: 0.85rem; color: var(--text-secondary);">
                Multiple recipients can be separated with commas. Note: Your Resend API key is managed securely via Render environment variables (`RESEND_API_KEY`).
            </div>
            <button
                type="submit"
                class="add-btn"
                id="save-config-btn"
                style="width: 100%"
                disabled={isSaving}
            >
                Save Config
            </button>
        </form>
    {/if}
    <div id="email-config-status" style="color: {statusColor}">{statusMsg}</div>
</section>
