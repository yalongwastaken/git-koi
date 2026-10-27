//! Reproducible koi development from account observations, independent of rendering.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::history::{
    AccountSnapshot, ConflictingRepository, ContributionHistoryError, ContributionSummary,
    ContributionWeek, LanguageColor, calculate_language_shares, summarize_contributions,
};

/// Derived inputs for appearance; source observations remain in the account snapshot.
#[derive(Debug, PartialEq)]
pub struct KoiDevelopment {
    /// The stable numeric GitHub account ID.
    pub identity_seed: u64,
    pub language_shares: BTreeMap<String, f64>,
    /// GitHub's color for each language in `language_shares`; `None` means it has no color.
    pub language_colors: BTreeMap<String, Option<LanguageColor>>,
    /// Unweighted contributions available to the renderer's growth curve.
    pub total_contributions: u64,
    pub observed_on: NaiveDate,
    pub streaks: StreakDevelopment,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StreakDevelopment {
    /// The earliest longest run, replaced only by a strictly longer run.
    pub longest: Vec<ContributionWeek>,
    /// None means no observed active week, not missing history.
    pub recent: Option<RecentStreak>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RecentStreak {
    Current(Vec<ContributionWeek>),
    Ended {
        weeks: Vec<ContributionWeek>,
        /// The Monday after the first wholly inactive week has completed.
        /// Further inactivity does not move this date or restart a fade.
        ended_on: NaiveDate,
    },
}

/// Derive one account's identity, language composition, and contribution development.
///
/// Any active day sustains a Monday-Sunday weekly streak. A completed inactive
/// week ends it; the observation week is still in progress and cannot end it.
/// Each streak retains weekly participation rather than just its length.
/// Returning activity replaces the recent streak without discarding the record.
///
/// History owns input validation and language eligibility. Its errors propagate;
/// unavailable history never becomes an undeveloped koi. Confirmed empty history
/// produces zero contributions and no streaks. Every counted language needs a
/// recorded color, so a snapshot without colors fails instead of producing an
/// uncolored koi. Inputs are borrowed without mutation; this operation performs
/// no I/O and reads no clock. It assigns no visual thresholds.
pub fn derive_koi_development(
    account_snapshot: &AccountSnapshot,
) -> Result<KoiDevelopment, KoiDevelopmentError> {
    let contribution_summary = summarize_contributions(account_snapshot)
        .map_err(KoiDevelopmentError::ContributionHistory)?;
    let language_shares =
        calculate_language_shares(account_snapshot).map_err(KoiDevelopmentError::Repositories)?;
    let recorded_colors = account_snapshot
        .language_colors
        .as_ref()
        .ok_or(KoiDevelopmentError::MissingLanguageColors)?;
    let language_colors = language_shares
        .keys()
        .map(|language_name| {
            recorded_colors
                .get(language_name)
                .map(|language_color| (language_name.clone(), *language_color))
                .ok_or_else(|| KoiDevelopmentError::MissingLanguageColor(language_name.clone()))
        })
        .collect::<Result<_, _>>()?;
    Ok(KoiDevelopment {
        identity_seed: account_snapshot.account_id,
        language_shares,
        language_colors,
        total_contributions: contribution_summary.total_contributions,
        observed_on: contribution_summary.observed_on,
        streaks: calculate_streak_development(&contribution_summary),
    })
}

/// Young koi show their language pigments pale until this total.
pub const FULL_COLOR_CONTRIBUTIONS: u64 = 500;
/// Young koi grow into the adult head, eyes, body width, and fins by this total.
pub const YOUNG_SHAPE_CONTRIBUTIONS: u64 = 1_500;
pub const ADULT_CONTRIBUTIONS: u64 = 12_750;
pub const MAX_GROWTH_CONTRIBUTIONS: u64 = 100_000;

/// Map total contributions to a growth fraction from zero through one.
/// Gains diminish until MAX_GROWTH_CONTRIBUTIONS; larger totals stay at one.
/// The caller supplies the observed total; elapsed time does not affect growth.
pub fn calculate_growth_fraction(total_contributions: u64) -> f64 {
    const CONTRIBUTION_SCALE: f64 = 50.0;
    let bounded_total = total_contributions.min(MAX_GROWTH_CONTRIBUTIONS);
    (1.0 + bounded_total as f64 / CONTRIBUTION_SCALE).log2()
        / (1.0 + MAX_GROWTH_CONTRIBUTIONS as f64 / CONTRIBUTION_SCALE).log2()
}

#[derive(Debug, Eq, PartialEq)]
pub enum KoiDevelopmentError {
    ContributionHistory(ContributionHistoryError),
    Repositories(ConflictingRepository),
    MissingLanguageColors,
    MissingLanguageColor(String),
}

impl fmt::Display for KoiDevelopmentError {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContributionHistory(history_error) => write!(output_formatter, "{history_error}"),
            Self::Repositories(repository_error) => write!(output_formatter, "{repository_error}"),
            Self::MissingLanguageColors => write!(
                output_formatter,
                "the snapshot has no language colors; collect it again with `git-koi collect`"
            ),
            Self::MissingLanguageColor(language_name) => write!(
                output_formatter,
                "the snapshot has no color for {language_name}; collect it again with `git-koi collect`"
            ),
        }
    }
}

impl Error for KoiDevelopmentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ContributionHistory(history_error) => Some(history_error),
            Self::Repositories(repository_error) => Some(repository_error),
            Self::MissingLanguageColors | Self::MissingLanguageColor(_) => None,
        }
    }
}

fn calculate_streak_development(contribution_summary: &ContributionSummary) -> StreakDevelopment {
    let mut longest_week_range = 0..0;
    let mut recent_week_range = 0..0;
    let mut current_start_week_index = None;
    let mut ended_on = None;
    let observation_week = contribution_summary.observed_on.iso_week();
    for (week_index, contribution_week) in contribution_summary.weeks.iter().enumerate() {
        if contribution_week.active_day_count > 0 {
            let start_week_index = *current_start_week_index.get_or_insert(week_index);
            recent_week_range = start_week_index..week_index + 1;
            ended_on = None;
            if recent_week_range.len() > longest_week_range.len() {
                longest_week_range = recent_week_range.clone();
            }
        } else if contribution_week.iso_week != observation_week
            && current_start_week_index.take().is_some()
        {
            let inactive_sunday = NaiveDate::from_isoywd_opt(
                contribution_week.iso_week.year(),
                contribution_week.iso_week.week(),
                Weekday::Sun,
            )
            .expect("a completed observation week has a Sunday");
            ended_on = Some(
                inactive_sunday
                    .succ_opt()
                    .expect("a completed week precedes the observation date"),
            );
        }
    }
    let recent_streak = if recent_week_range.is_empty() {
        None
    } else {
        let recent_weeks = contribution_summary.weeks[recent_week_range].to_vec();
        Some(match ended_on {
            Some(ended_on) => RecentStreak::Ended {
                weeks: recent_weeks,
                ended_on,
            },
            None => RecentStreak::Current(recent_weeks),
        })
    };
    StreakDevelopment {
        longest: contribution_summary.weeks[longest_week_range].to_vec(),
        recent: recent_streak,
    }
}
