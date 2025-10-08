use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc, Weekday};
use indoc::indoc;
use phf::phf_map;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write,
    fs::{self},
    path::Path,
};
use uuid::Uuid;

static SUBJECTS: phf::Map<&'static str, &'static str> = phf_map! {
    "Academic Learning and Success" => "ALSU",
    "Biology" => "BIOL",
    "Business" => "BUSI",
    "Chemistry" => "CHEM",
    "Communications" => "COMM",
    "Computer Science" => "CSCI",
    "Criminology and Justice" => "CRMN",
    "Curriculum Studies" => "CURS",
    "Economics" => "ECON",
    "Education" => "EDUC",
    "Educational Studies and Digital Technology" => "AEDT",
    "Electrical Engineering" => "ELEE",
    "Energy Systems and Nuclear Science" => "ESNS",
    "Engineering" => "ENGR",
    "Environmental Science" => "ENVS",
    "Forensic Science" => "FSCI",
    "Health Science" => "HLSC",
    "Indigenous Studies" => "INDG",
    "Information Technology" => "INFR",
    "Integrated Mathematics and Computer Science" => "IMCS",
    "Kinesiology" => "KINE",
    "Legal Studies" => "LGLS",
    "Liberal Studies" => "LBAT",
    "Manufacturing Engineering" => "MANE",
    "Mathematics" => "MATH",
    "Mechanical Engineering" => "MECE",
    "Mechatronics Engineering" => "METE",
    "Medical Laboratory Science" => "MLSC",
    "Neuroscience" => "NSCI",
    "Nuclear" => "NUCL",
    "Nursing" => "NURS",
    "Physics" => "PHY",
    "Political Science" => "POSC",
    "Psychology" => "PSYC",
    "Radiation Science" => "RADI",
    "Science" => "SCIE",
    "Science Co-op" => "SCCO",
    "Science Co-op Work Term" => "SCCO",
    "Social Science" => "SSCI",
    "Sociology" => "SOCI",
    "Software Engineering" => "SOFE",
    "Statistics" => "STAT",
    "Sustainable Energy Systems" => "ENSY",
};

#[derive(Debug)]
struct DateRange {
    start_date: NaiveDate,
    end_date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    weekday: Weekday,
    location: String,
    building: String,
    room: String,
}

#[derive(Debug)]
struct Class {
    name: String,
    code: String,
    date_ranges: Vec<DateRange>,
    instructor: String,
    crn: String,
    class_type: String,
}

enum Browser {
    Chromium,
    Firefox,
}

#[derive(Serialize, Deserialize)]
pub struct Parser {
    pub course_summary_re: String,
    pub course_name_re: String,
    pub date_re: String,
    pub time_re: String,
    pub message_re: String,
    pub crn_re: String,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            course_summary_re: r"^.+?\t(?<subject>[A-Z]{4}) \d{4}U, .+?\t(?<crn>\d{5})".to_string(),
            course_name_re: r"^(?<name>.+?) \| (?<subject>.+?) (?<code>\d+U)".to_string(),
            date_re: r"^(?<start>[\d/]+) -- (?<end>[\d/]+)(?:\s+(?<weekday>\w+))?".to_string(),
            time_re: r"^\s+(?<start>\d+:\d+ \w+) - (?<end>\d+:\d+ \w+).+?Location: (?<location>.+?) Building: (?<building>.+?) Room: (?<room>.+)".to_string(),
            message_re: r"\| Schedule Type: (?<class_type>.+?) \|".to_string(),
            crn_re: r"^CRN: (?<crn>\d{5})".to_string(),
        }
    }
}

impl Parser {
    fn parse_data(&self, raw_data: &str) -> Vec<Class> {
        let course_summary_re = Regex::new(&self.course_summary_re).unwrap();
        let course_name_re = Regex::new(&self.course_name_re).unwrap();
        let date_re = Regex::new(&self.date_re).unwrap();
        let time_re = Regex::new(&self.time_re).unwrap();
        let message_re = Regex::new(&self.message_re).unwrap();
        let crn_re = Regex::new(&self.crn_re).unwrap();

        // wHY ARE THEY USING NO-BREAK SPACES NOW
        let mut lines = raw_data.lines().map(|l| l.replace('\u{a0}', " "));

        let mut crn_short_subjects: HashMap<String, String> = HashMap::new();
        let browser = 'browser: {
            for line in lines.by_ref() {
                // in case long subject names keep changing
                // also try to get the short code from the summary at the start of the data
                if let Some(caps) = course_summary_re.captures(&line) {
                    let short_subject = caps.name("subject").unwrap().as_str();
                    let crn = caps.name("crn").unwrap().as_str();
                    crn_short_subjects.insert(crn.to_owned(), short_subject.to_owned());
                }

                match &*line {
                    "Schedule" => break 'browser Browser::Chromium,
                    "    Schedule" => break 'browser Browser::Firefox,
                    _ => (),
                }
            }
            panic!("Failed to find Schedule line to determine browser")
        };

        // skip unneeded prelude
        while !lines
            .next()
            .expect("Failed to find start of schedule")
            .starts_with("Class Schedule for ")
        {}

        let mut output = Vec::new();

        while let Some(course_name_line) = lines.next() {
            // handle extra newlines at the end
            if course_name_line.is_empty() {
                break;
            }

            // parse course name and code
            let course_name_caps =
                course_name_re
                    .captures(&course_name_line)
                    .unwrap_or_else(|| {
                        panic!("Failed to match course name line: {}", course_name_line)
                    });
            let name = course_name_caps.name("name").unwrap().as_str().to_string();
            let subject = course_name_caps.name("subject").unwrap().as_str();
            let code_number = course_name_caps.name("code").unwrap().as_str();

            // skip "Registered" line
            lines.next();

            // why did they CHANGE THE FORMAT
            // JUST TO MOVE THIS BOX TO THE TOP
            let message_line = lines.next().unwrap();
            let message_caps = message_re
                .captures(&message_line)
                .unwrap_or_else(|| panic!("Failed to parse message line: {}", message_line));

            // parse date ranges
            let mut date_ranges = Vec::new();
            let instructor = loop {
                let date_line = lines.next().unwrap();
                let date_caps = match date_re.captures(&date_line) {
                    Some(caps) => caps,
                    None => break date_line,
                };

                let start_date = date_caps.name("start").unwrap().as_str();
                let start_date = NaiveDate::parse_from_str(start_date, "%m/%d/%Y")
                    .unwrap_or_else(|e| panic!("Failed to parse date: {}\n{}", start_date, e));

                let end_date = date_caps.name("end").unwrap().as_str();
                let end_date = NaiveDate::parse_from_str(end_date, "%m/%d/%Y")
                    .unwrap_or_else(|e| panic!("Failed to parse date: {}\n{}", end_date, e));

                let weekday = match browser {
                    Browser::Firefox => lines.next().unwrap(),
                    Browser::Chromium => date_caps.name("weekday").unwrap().as_str().to_string(),
                };
                if weekday == "None" {
                    lines.nth(match browser {
                        Browser::Chromium => 7,
                        Browser::Firefox => 9,
                    });
                    continue;
                }
                let weekday = weekday
                    .parse::<Weekday>()
                    .unwrap_or_else(|_| panic!("Failed to parse weekday: {}", weekday));

                // skip day abbreviations
                lines.nth(match browser {
                    Browser::Chromium => 6,
                    Browser::Firefox => 8,
                });

                let time_line = lines.next().unwrap();
                let time_caps = time_re
                    .captures(&time_line)
                    .unwrap_or_else(|| panic!("Failed to parse time line: {}", time_line));

                let start_time = time_caps.name("start").unwrap().as_str();
                let start_time = NaiveTime::parse_from_str(start_time, "%I:%M %p")
                    .unwrap_or_else(|e| panic!("Failed to parse time: {}\n{}", start_time, e));

                let end_time = time_caps.name("end").unwrap().as_str();
                let end_time = NaiveTime::parse_from_str(end_time, "%I:%M %p")
                    .unwrap_or_else(|e| panic!("Failed to parse time: {}\n{}", end_time, e));

                let location = time_caps.name("location").unwrap().as_str().to_string();
                let building = time_caps.name("building").unwrap().as_str().to_string();
                let room = time_caps.name("room").unwrap().as_str().to_string();

                date_ranges.push(DateRange {
                    start_date,
                    end_date,
                    start_time,
                    end_time,
                    weekday,
                    location,
                    building,
                    room,
                });
            };

            let crn_line = lines.next().unwrap();

            let short_subject = SUBJECTS
                .get(subject)
                .map(|s| (*s).to_owned())
                .or_else(|| {
                    crn_re
                        .captures(&crn_line)
                        .and_then(|caps| caps.name("crn"))
                        .and_then(|crn| crn_short_subjects.get(crn.as_str()))
                        .cloned()
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Failed to get short subject code for subject: {}\nFound subjects: {:#?}",
                        subject, crn_short_subjects
                    )
                });
            let code = format!("{short_subject} {code_number}");

            output.push(Class {
                name,
                code,
                date_ranges,
                instructor,
                crn: crn_line,
                class_type: message_caps
                    .name("class_type")
                    .unwrap()
                    .as_str()
                    .to_string(),
            })
        }

        output
    }
}

fn tzid(datetime: NaiveDateTime) -> String {
    format!("TZID=America/Toronto:{}", datetime.format("%Y%m%dT%H%M%S"))
}

fn fold_calendar(calendar: &mut String) {
    let mut to_insert = Vec::new();
    let mut line_length = 0;
    for (index, c) in calendar.chars().enumerate() {
        if c == '\n' {
            line_length = 0;
        } else {
            line_length += 1;
            if line_length >= 74 {
                to_insert.push(index);
                line_length = 0;
            }
        }
    }
    for index in to_insert.iter().rev() {
        calendar.insert_str(*index, "\n ");
    }
}

pub fn generate(
    output_folder: impl AsRef<Path>,
    parser: &Parser,
    data: &str,
    exdate: HashSet<NaiveDate>,
) -> usize {
    let data = parser.parse_data(data);

    println!("Data: {:#?}\nExcluded dates: {:?}", data, exdate);

    let mut calendars = HashMap::new();
    let mut summary: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();

    for class in &data {
        let calendar = calendars
            .entry(class.class_type.clone())
            .or_insert_with(|| {
                indoc! {"
                    BEGIN:VCALENDAR
                    VERSION:2.0
                    PRODID:MYCAMPUS-CALENDAR-RS
                    CALSCALE:GREGORIAN
                    BEGIN:VTIMEZONE
                    TZID:America/Toronto
                    LAST-MODIFIED:20201011T015911Z
                    TZURL:http://tzurl.org/zoneinfo-outlook/America/Toronto
                    X-LIC-LOCATION:America/Toronto
                    BEGIN:DAYLIGHT
                    TZNAME:EDT
                    TZOFFSETFROM:-0500
                    TZOFFSETTO:-0400
                    DTSTART:19700308T020000
                    RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU
                    END:DAYLIGHT
                    BEGIN:STANDARD
                    TZNAME:EST
                    TZOFFSETFROM:-0400
                    TZOFFSETTO:-0500
                    DTSTART:19701101T020000
                    RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU
                    END:STANDARD
                    END:VTIMEZONE
                "}
                .to_string()
            });
        let class_summary_count = summary
            .entry(class.name.clone())
            .or_default()
            .entry(class.class_type.clone())
            .or_default();

        for date_range in &class.date_ranges {
            let first_date = date_range.start_date
                + Duration::days(
                    (date_range.weekday.num_days_from_sunday() as i32
                        - date_range.start_date.weekday().num_days_from_sunday() as i32)
                        .rem_euclid(7)
                        .into(),
                );
            let exdate = format!(
                "EXDATE;TZID=America/Toronto:{}",
                exdate
                    .iter()
                    .map(|d| d
                        .and_time(date_range.start_time)
                        .format("%Y%m%dT%H%M%S")
                        .to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );

            write!(
                calendar,
                indoc! {r#"
                    BEGIN:VEVENT
                    DTSTAMP:{dtstamp}
                    UID:{uid}
                    DTSTART;{dtstart}
                    DTEND;{dtend}
                    RRULE:FREQ=WEEKLY;TZID=America/Toronto;UNTIL={until}
                    {exdate}
                    SUMMARY:{name}
                    DESCRIPTION:Campus: {location}\nCode: {code}\n{crn}\n{instructor}
                    LOCATION:{building} - {room}
                    END:VEVENT
                "#},
                dtstamp = Utc::now().format("%Y%m%dT%H%M%SZ"),
                uid = Uuid::new_v4(),
                dtstart = tzid(first_date.and_time(date_range.start_time)),
                dtend = tzid(first_date.and_time(date_range.end_time)),
                until = date_range
                    .end_date
                    .and_hms_opt(23, 59, 59)
                    .unwrap()
                    .format("%Y%m%dT%H%M%S"),
                exdate = exdate,
                name = class.name,
                code = class.code,
                crn = class.crn,
                instructor = class.instructor,
                location = date_range.location,
                building = date_range.building,
                room = date_range.room,
            )
            .ok();

            *class_summary_count += 1;
        }
    }

    for (name, calendar) in &mut calendars {
        calendar.push_str("END:VCALENDAR");
        fold_calendar(calendar);
        *calendar = calendar.replace('\n', "\r\n");

        let output_path = output_folder.as_ref().join(format!(
            "{}.ics",
            name.replace(
                ['/', '\\', '<', '>', ':', '"', '\'', '|', '?', '*', ' ', '\r', '\n', '\0'],
                "_"
            )
        ));
        println!("Writing calendar: {}", output_path.display());
        fs::write(output_path, calendar).ok();
    }

    let max_name_len = summary.keys().map(|n| n.len()).max().unwrap();
    for (name, class_summary) in summary {
        println!(
            "{:indent$}{} → {}",
            "",
            name,
            class_summary
                .iter()
                .map(|(class_type, count)| format!("{}: {}", class_type, count))
                .collect::<Vec<String>>()
                .join(", "),
            indent = max_name_len - name.len()
        );
    }

    let n = calendars.len();
    println!("Wrote {n} .ics file(s).");
    n
}
