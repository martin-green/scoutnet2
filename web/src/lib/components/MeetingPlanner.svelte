<script lang="ts">
	import { api, ApiError, fmtWeekday, meetingKindLabels, type Body, type Meeting } from '#lib/api/client.ts';

	let {
		body,
		term,
		onclose,
		oncreated
	}: { body: Body; term: string; onclose: () => void; oncreated: (m: Meeting[]) => void } = $props();

	const iso = (d: Date) => d.toISOString().slice(0, 10);
	const addDays = (d: string, n: number) => {
		const x = new Date(`${d}T12:00:00Z`);
		x.setUTCDate(x.getUTCDate() + n);
		return iso(x);
	};

	// Förslag: avdelningens vanliga mötesdag från i dag (eller terminsstart), fram till strax före terminens slut.
	function defaults() {
		const m = /^(VT|HT)\s*(\d{4})$/i.exec(term);
		const year = Number(m?.[2] ?? new Date().getFullYear());
		const autumn = (m?.[1] ?? 'HT').toUpperCase() === 'HT';
		const termStart = autumn ? `${year}-08-17` : `${year}-01-12`;
		const termEnd = autumn ? `${year}-12-13` : `${year}-06-07`;
		let first = iso(new Date()) > termStart ? iso(new Date()) : termStart;
		const weekday = body.meeting?.weekday;
		if (weekday) {
			for (let i = 0; i < 7; i++) {
				const d = addDays(first, i);
				const wd = new Date(`${d}T12:00:00Z`).getUTCDay() || 7;
				if (wd === weekday) {
					first = d;
					break;
				}
			}
		}
		return { first, termEnd, time: body.meeting?.start ?? '18:00', duration: body.meeting?.duration_min ?? 90, location: body.meeting?.location ?? '' };
	}
	const init = defaults();

	let name = $state('Möte');
	let kind = $state('meeting');
	let date = $state(init.first);
	let time = $state(init.time);
	let duration = $state(init.duration);
	let location = $state(init.location);
	let repeat = $state(true);
	let until = $state(init.termEnd);
	let skipped = $state<Record<string, boolean>>({});
	let busy = $state(false);
	let error = $state<string | null>(null);

	const dates = $derived.by(() => {
		if (!date) return [];
		if (!repeat || !until || until < date) return [date];
		const out: string[] = [];
		for (let d = date; d <= until && out.length < 61; d = addDays(d, 7)) out.push(d);
		return out;
	});
	const chosen = $derived(dates.filter((d) => !skipped[d]));

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = null;
		try {
			const created = await api.createMeetings(`troop-${body.number}`, {
				name: name.trim(),
				kind,
				date,
				time,
				duration_min: Number(duration),
				location: location.trim() || null,
				repeat_until: repeat && dates.length > 1 ? until : null,
				skip_dates: repeat ? dates.filter((d) => skipped[d]) : []
			});
			oncreated(created);
		} catch (err) {
			error = err instanceof ApiError ? (err.problem?.detail ?? err.message) : String(err);
		} finally {
			busy = false;
		}
	}
</script>

<div class="backdrop" onclick={onclose} aria-hidden="true"></div>
<div class="dialog card" role="dialog" aria-modal="true" aria-labelledby="plan-title">
	<h2 id="plan-title">Planera möten – {body.name}</h2>
	<form onsubmit={submit}>
		<div class="grid">
			<label>Namn <input class="input" bind:value={name} required maxlength="100" /></label>
			<label>
				Typ
				<select class="input" bind:value={kind}>
					{#each Object.entries(meetingKindLabels) as [k, l] (k)}<option value={k}>{l}</option>{/each}
				</select>
			</label>
			<label>Första datum <input class="input" type="date" bind:value={date} required /></label>
			<label>Starttid <input class="input" type="time" bind:value={time} required /></label>
			<label>Längd (minuter) <input class="input" type="number" min="5" max="20160" bind:value={duration} required /></label>
			<label>Plats <input class="input" bind:value={location} placeholder="t.ex. Scoutstugan" /></label>
		</div>

		<label class="check"><input type="checkbox" bind:checked={repeat} /> Upprepa varje vecka till och med</label>
		{#if repeat}
			<input class="input until" type="date" bind:value={until} min={date} aria-label="Upprepa till och med" />
			{#if dates.length > 1}
				<p class="muted small">Bocka ur lov och andra veckor utan möte. Namnet får veckonummer, t.ex. "{name} v.43".</p>
				<ul class="dates">
					{#each dates as d (d)}
						<li>
							<label class:off={skipped[d]}>
								<input type="checkbox" checked={!skipped[d]} onchange={(e) => (skipped[d] = !e.currentTarget.checked)} />
								{fmtWeekday(d)}
							</label>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}

		{#if dates.length > 60}<p class="error">Högst 60 möten per serie.</p>{/if}
		{#if error}<p class="error">{error}</p>{/if}

		<div class="actions">
			<button type="button" class="btn" onclick={onclose}>Avbryt</button>
			<button class="btn primary" disabled={busy || chosen.length === 0 || dates.length > 60}>
				{busy ? 'Skapar…' : `Skapa ${chosen.length} ${chosen.length === 1 ? 'möte' : 'möten'}`}
			</button>
		</div>
	</form>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 35%);
		z-index: 40;
	}
	.dialog {
		position: fixed;
		z-index: 50;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: min(640px, calc(100vw - 24px));
		max-height: calc(100vh - 40px);
		overflow-y: auto;
	}
	h2 {
		margin-top: 0;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.88rem;
		color: var(--muted);
	}
	label .input {
		color: var(--text);
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: var(--space-2);
		color: var(--text);
	}
	.until {
		margin: 6px 0 var(--space-2);
		max-width: 200px;
	}
	.dates {
		list-style: none;
		padding: 0;
		margin: 0 0 var(--space-3);
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
		gap: 2px var(--space-3);
	}
	.dates label {
		flex-direction: row;
		align-items: center;
		gap: 6px;
		color: var(--text);
		font-size: 0.9rem;
	}
	.dates label.off {
		color: var(--muted);
		text-decoration: line-through;
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.small {
		font-size: 0.82rem;
	}
	.error {
		color: var(--sn-red);
	}
</style>
