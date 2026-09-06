"""ContextStore: insert, prune-at-50, per-tenant/per-user isolation, and restart durability — the
properties `session_warmup` depends on to serve a real `recent_tasks[]` instead of always `[]`.
"""

from orb_relay.proxy.schemas import AtomizerOutput, AtomizerStep
from orb_relay.store.context_store import MAX_RECENT_TASKS_PER_USER, ContextStore

STEPS = AtomizerOutput(
    steps=[AtomizerStep(step_text="Open the tax portal", est_min=1, done_signal="portal on screen")],
    steps_total=1,
)


def make_store(tmp_path) -> ContextStore:
    return ContextStore(tmp_path / "context.db")


def test_insert_then_read_back_round_trips_task_and_steps(tmp_path) -> None:
    store = make_store(tmp_path)
    store.insert_task(
        tenant_id="t1", user_id="u1", task_text="file my taxes", steps=STEPS, created_at=1000.0
    )

    records = store.recent_tasks(tenant_id="t1", user_id="u1")

    assert len(records) == 1
    assert records[0].task_text == "file my taxes"
    assert records[0].steps == STEPS
    assert records[0].created_at == 1000.0


def test_recent_tasks_for_unknown_user_is_empty(tmp_path) -> None:
    store = make_store(tmp_path)
    assert store.recent_tasks(tenant_id="t1", user_id="nobody") == []


def test_recent_tasks_ordered_most_recent_first(tmp_path) -> None:
    store = make_store(tmp_path)
    for i in range(5):
        store.insert_task(
            tenant_id="t1", user_id="u1", task_text=f"task {i}", steps=STEPS, created_at=float(i)
        )

    records = store.recent_tasks(tenant_id="t1", user_id="u1")

    assert [r.task_text for r in records] == ["task 4", "task 3", "task 2", "task 1", "task 0"]


def test_prune_caps_at_max_recent_tasks_per_user_dropping_oldest_first(tmp_path) -> None:
    store = make_store(tmp_path)
    total_inserted = MAX_RECENT_TASKS_PER_USER + 10

    for i in range(total_inserted):
        store.insert_task(
            tenant_id="t1", user_id="u1", task_text=f"task {i}", steps=STEPS, created_at=float(i)
        )

    records = store.recent_tasks(tenant_id="t1", user_id="u1", limit=total_inserted)

    assert len(records) == MAX_RECENT_TASKS_PER_USER
    # The oldest 10 (task 0..9) must have been pruned; the newest must survive.
    task_texts = {r.task_text for r in records}
    assert "task 0" not in task_texts
    assert "task 9" not in task_texts
    assert "task 10" in task_texts
    assert f"task {total_inserted - 1}" in task_texts


def test_different_tenants_are_isolated_even_with_the_same_user_id(tmp_path) -> None:
    store = make_store(tmp_path)
    store.insert_task(
        tenant_id="tenant-a", user_id="u1", task_text="tenant a task", steps=STEPS, created_at=1.0
    )
    store.insert_task(
        tenant_id="tenant-b", user_id="u1", task_text="tenant b task", steps=STEPS, created_at=1.0
    )

    a_records = store.recent_tasks(tenant_id="tenant-a", user_id="u1")
    b_records = store.recent_tasks(tenant_id="tenant-b", user_id="u1")

    assert [r.task_text for r in a_records] == ["tenant a task"]
    assert [r.task_text for r in b_records] == ["tenant b task"]


def test_different_users_within_the_same_tenant_are_isolated(tmp_path) -> None:
    store = make_store(tmp_path)
    store.insert_task(
        tenant_id="t1", user_id="u1", task_text="u1 task", steps=STEPS, created_at=1.0
    )
    store.insert_task(
        tenant_id="t1", user_id="u2", task_text="u2 task", steps=STEPS, created_at=1.0
    )

    u1_records = store.recent_tasks(tenant_id="t1", user_id="u1")
    u2_records = store.recent_tasks(tenant_id="t1", user_id="u2")

    assert [r.task_text for r in u1_records] == ["u1 task"]
    assert [r.task_text for r in u2_records] == ["u2 task"]


def test_pruning_one_user_does_not_touch_another_users_rows(tmp_path) -> None:
    store = make_store(tmp_path)
    for i in range(MAX_RECENT_TASKS_PER_USER + 5):
        store.insert_task(
            tenant_id="t1", user_id="busy", task_text=f"task {i}", steps=STEPS, created_at=float(i)
        )
    store.insert_task(
        tenant_id="t1", user_id="quiet", task_text="the only one", steps=STEPS, created_at=0.0
    )

    quiet_records = store.recent_tasks(tenant_id="t1", user_id="quiet")

    assert [r.task_text for r in quiet_records] == ["the only one"]


def test_data_survives_a_new_store_instance_against_the_same_file(tmp_path) -> None:
    """Simulates a relay restart: a fresh `ContextStore` pointed at the same db file must see the
    prior process's writes.
    """
    db_path = tmp_path / "context.db"
    ContextStore(db_path).insert_task(
        tenant_id="t1", user_id="u1", task_text="before restart", steps=STEPS, created_at=1.0
    )

    reopened = ContextStore(db_path)
    records = reopened.recent_tasks(tenant_id="t1", user_id="u1")

    assert [r.task_text for r in records] == ["before restart"]
