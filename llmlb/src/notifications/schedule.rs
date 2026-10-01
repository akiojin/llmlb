//! 壁時計基準の日次スケジュール計算
//!
//! プロセス起動からの経過時間ではなく、サーバーのローカル時刻と「最終送信日」から
//! 次回実行時刻を求める。再起動しても送信時刻はずれず、同じ日に二度実行されない。

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// `HH:MM`（24 時間表記）を時刻として解釈する
pub fn parse_time_of_day(value: &str) -> Option<NaiveTime> {
    let is_hh_mm = matches!(
        value.as_bytes(),
        [h1, h2, b':', m1, m2] if [h1, h2, m1, m2].iter().all(|digit| digit.is_ascii_digit())
    );
    if !is_hh_mm {
        return None;
    }
    NaiveTime::parse_from_str(value, "%H:%M").ok()
}

/// 次回の実行時刻を求める
///
/// - 当日分を送信済み（`last_sent == 当日`）なら翌日の `time_of_day`
/// - 未送信で予定時刻を過ぎていれば `now`（停止中に過ぎた当日分を 1 回だけ送る）
/// - 未送信で予定時刻より前なら当日の `time_of_day`
pub fn next_run(
    now: NaiveDateTime,
    time_of_day: NaiveTime,
    last_sent: Option<NaiveDate>,
) -> NaiveDateTime {
    let today = now.date();
    if last_sent == Some(today) {
        // 日付の上限（262142 年）を超える場合だけ当日に留まる
        let next_day = today.succ_opt().unwrap_or(today);
        return next_day.and_time(time_of_day);
    }
    now.max(today.and_time(time_of_day))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn day(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    fn nine() -> NaiveTime {
        NaiveTime::from_hms_opt(9, 0, 0).unwrap()
    }

    #[test]
    fn parse_time_of_day_accepts_hh_mm() {
        assert_eq!(parse_time_of_day("09:00"), Some(nine()));
        assert_eq!(parse_time_of_day("00:00"), NaiveTime::from_hms_opt(0, 0, 0));
        assert_eq!(
            parse_time_of_day("23:59"),
            NaiveTime::from_hms_opt(23, 59, 0)
        );
    }

    #[test]
    fn parse_time_of_day_rejects_other_formats() {
        for invalid in [
            "", "9:00", "24:00", "12:60", "09:00:00", "ab:cd", " 09:00", "09:00 ", "0900", "09-00",
        ] {
            assert_eq!(parse_time_of_day(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn waits_until_todays_time_when_not_sent_yet() {
        assert_eq!(
            next_run(at("2026-10-01 08:59:59"), nine(), None),
            at("2026-10-01 09:00:00")
        );
        assert_eq!(
            next_run(at("2026-10-01 00:00:00"), nine(), Some(day("2026-09-30"))),
            at("2026-10-01 09:00:00")
        );
    }

    #[test]
    fn is_due_at_and_after_the_scheduled_time_when_not_sent_yet() {
        for now in [
            "2026-10-01 09:00:00",
            "2026-10-01 10:15:00",
            "2026-10-01 23:59:59",
        ] {
            assert_eq!(next_run(at(now), nine(), None), at(now), "{now}");
            assert_eq!(
                next_run(at(now), nine(), Some(day("2026-09-30"))),
                at(now),
                "{now}"
            );
        }
    }

    /// 送信済みなら、いつ再起動しても次回は翌日の同時刻（起動時刻に引きずられない）
    #[test]
    fn next_run_after_sending_is_tomorrow_at_the_same_time_regardless_of_now() {
        for now in [
            "2026-10-01 09:00:01",
            "2026-10-01 13:37:42",
            "2026-10-01 23:59:59",
        ] {
            assert_eq!(
                next_run(at(now), nine(), Some(day("2026-10-01"))),
                at("2026-10-02 09:00:00"),
                "{now}"
            );
        }
    }

    #[test]
    fn rolls_over_month_and_year_boundaries() {
        assert_eq!(
            next_run(at("2026-12-31 09:00:05"), nine(), Some(day("2026-12-31"))),
            at("2027-01-01 09:00:00")
        );
    }

    /// 送信後に送信時刻を後ろへ変更しても、同じ日には再実行しない
    #[test]
    fn changing_the_time_after_sending_does_not_rerun_on_the_same_day() {
        let evening = NaiveTime::from_hms_opt(18, 0, 0).unwrap();
        assert_eq!(
            next_run(at("2026-10-01 17:00:00"), evening, Some(day("2026-10-01"))),
            at("2026-10-02 18:00:00")
        );
        assert_eq!(
            next_run(at("2026-10-01 18:30:00"), evening, Some(day("2026-10-01"))),
            at("2026-10-02 18:00:00")
        );
    }

    /// 時計が巻き戻って最終送信日が未来になっても、送信が止まり続けない
    #[test]
    fn a_last_sent_date_in_the_future_does_not_block_sending() {
        assert_eq!(
            next_run(at("2026-10-01 09:30:00"), nine(), Some(day("2026-10-05"))),
            at("2026-10-01 09:30:00")
        );
    }
}
