use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc, Weekday};
use indoc::indoc;
use phf::phf_map;
use regex::Regex;
use std::{
    collections::HashMap,
    fs,
    fs::File,
    io::{prelude::*, BufReader},
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
    "Indigenous" => "INDG",
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

fn parse_data(filename: impl AsRef<Path>) -> Vec<Class> {
    let file = File::open(filename).unwrap_or_else(|e| panic!("Couldn't open data file: {}", e));
    let mut lines = BufReader::new(file).lines().map(|l| l.unwrap());

    // determine what browser was used to generate the data file
    let browser = loop {
        let next = lines
            .next()
            .expect("Failed to find Schedule line to determine browser");
        match &*next {
            "Schedule" => break Browser::Chromium,
            "    Schedule" => break Browser::Firefox,
            _ => (),
        }
    };

    // skip unneeded prelude
    while !lines
        .next()
        .expect("Failed to find start of schedule")
        .starts_with("Class Schedule for ")
    {}

    let mut output = Vec::new();
    let course_name_re = Regex::new(r"^(.+?) \| (.+?) (\d+U)").unwrap();
    let date_re = Regex::new(r"^([\d/]+) -- ([\d/]+)(?:\s+(\w+))?").unwrap();
    let time_re = Regex::new(
        r"^\s+(\d+:\d+ \w+) - (\d+:\d+ \w+).+?Location: (?P<location>.+?) Building: (?P<building>.+?) Room: (?P<room>.+)",
    )
    .unwrap();
    let message_re = Regex::new(r"\| Schedule Type: (?P<class_type>.+?) \|").unwrap();

    while let Some(course_name_line) = lines.next() {
        // parse course name and code
        let course_name_caps = course_name_re
            .captures(&course_name_line)
            .unwrap_or_else(|| panic!("Failed to match course name line: {}", course_name_line));
        let name = course_name_caps.get(1).unwrap().as_str().to_string();
        let subject = course_name_caps.get(2).unwrap().as_str();
        let code = format!(
            "{} {}",
            SUBJECTS.get(subject).unwrap_or_else(|| panic!(
                "Failed to get short subject code for subject: {}",
                subject
            )),
            course_name_caps.get(3).unwrap().as_str()
        );

        // skip "Registered" line
        lines.next();

        // parse date ranges
        let mut date_ranges = Vec::new();
        let instructor = loop {
            let date_line = lines.next().unwrap();
            let date_caps = match date_re.captures(&date_line) {
                Some(caps) => caps,
                None => break date_line,
            };

            let start_date = date_caps.get(1).unwrap().as_str();
            let start_date = NaiveDate::parse_from_str(start_date, "%m/%d/%Y")
                .unwrap_or_else(|e| panic!("Failed to parse date: {}\n{}", start_date, e));

            let end_date = date_caps.get(2).unwrap().as_str();
            let end_date = NaiveDate::parse_from_str(end_date, "%m/%d/%Y")
                .unwrap_or_else(|e| panic!("Failed to parse date: {}\n{}", end_date, e));

            let weekday = match browser {
                Browser::Firefox => lines.next().unwrap(),
                Browser::Chromium => date_caps.get(3).unwrap().as_str().to_string(),
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

            let start_time = time_caps.get(1).unwrap().as_str();
            let start_time = NaiveTime::parse_from_str(start_time, "%I:%M %p")
                .unwrap_or_else(|e| panic!("Failed to parse time: {}\n{}", start_time, e));

            let end_time = time_caps.get(2).unwrap().as_str();
            let end_time = NaiveTime::parse_from_str(end_time, "%I:%M %p")
                .unwrap_or_else(|e| panic!("Failed to parse time: {}\n{}", end_time, e));

            let location = format!(
                "{} - {} - {}",
                time_caps.name("location").unwrap().as_str(),
                time_caps.name("building").unwrap().as_str(),
                time_caps.name("room").unwrap().as_str(),
            );

            date_ranges.push(DateRange {
                start_date,
                end_date,
                start_time,
                end_time,
                weekday,
                location,
            });
        };

        let crn = lines.next().unwrap();

        let message_line = lines.next().unwrap();
        let message_caps = message_re
            .captures(&message_line)
            .unwrap_or_else(|| panic!("Failed to parse message line: {}", message_line));

        output.push(Class {
            name,
            code,
            date_ranges,
            instructor,
            crn,
            class_type: message_caps
                .name("class_type")
                .unwrap()
                .as_str()
                .to_string(),
        })
    }

    output
}

fn parse_exdate(filename: impl AsRef<Path>) -> Vec<NaiveDate> {
    let file = File::open(filename).unwrap_or_else(|e| panic!("Couldn't open data file: {}", e));
    BufReader::new(file)
        .lines()
        .map(|l| {
            let l = l.unwrap();
            let split = l
                .split_once(" - ")
                .unwrap_or_else(|| panic!("Failed to parse exdate line: {}", l));

            let start_date = split
                .0
                .parse::<NaiveDate>()
                .unwrap_or_else(|e| panic!("Failed to parse exdate: {}\n{}", split.0, e));

            let end_date = split
                .1
                .parse::<NaiveDate>()
                .unwrap_or_else(|e| panic!("Failed to parse exdate: {}\n{}", split.1, e));

            let mut dates = Vec::new();
            for date in start_date.iter_days() {
                dates.push(date);
                if date == end_date {
                    break;
                }
            }
            dates
        })
        .flatten()
        .collect()
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

fn main() {
    let data = parse_data("data.txt");
    let exdate = parse_exdate("exdate.txt");

    println!("Data: {:#?}\nExcluded dates: {:?}", data, exdate);

    let mut calendars = HashMap::new();

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
            calendar.push_str(&format!(
                indoc! {r#"
                    BEGIN:VEVENT
                    DTSTAMP:{}
                    UID:{}
                    DTSTART;{}
                    DTEND;{}
                    RRULE:FREQ=WEEKLY;TZID=America/Toronto;UNTIL={}
                    {}
                    SUMMARY:{} - {}
                    DESCRIPTION:{}\n{}
                    LOCATION:{}
                    END:VEVENT
                "#},
                Utc::now().format("%Y%m%dT%H%M%SZ"),
                Uuid::new_v4(),
                tzid(first_date.and_time(date_range.start_time)),
                tzid(first_date.and_time(date_range.end_time)),
                date_range
                    .end_date
                    .and_hms(23, 59, 59)
                    .format("%Y%m%dT%H%M%S"),
                exdate,
                class.code,
                class.name,
                class.crn,
                class.instructor,
                date_range.location
            ));
        }
    }

    for (name, calendar) in &mut calendars {
        calendar.push_str("END:VCALENDAR");
        fold_calendar(calendar);
        *calendar = calendar.replace("\n", "\r\n");
        fs::write(format!("{}.ics", name), calendar).ok();
    }

    println!("Wrote .ics file(s).");
}
