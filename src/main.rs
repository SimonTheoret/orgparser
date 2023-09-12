// Simple utility to automate the process of reminder for emacs orgmode.
// This application parse the org mode file(s) and look for the next scheduled event/todo
// It generates a notification n minutes before the event takes place.

use parsing::generate_todos;

/// Module to iterate through the org directory and find .org files.
/// It ignores hidden directories (directories startign with ".")
mod parsing {
    use chrono::{NaiveDateTime, ParseError};
    use rayon::prelude::*;
    use std::fmt;
    use std::path::PathBuf;
    use tokio::fs::read_to_string;
    use walkdir::{DirEntry, WalkDir};

    /// Return the list of .org files in the org directory
    fn get_org_entries(org_dir: &str) -> Vec<PathBuf> {
        let walker = WalkDir::new(org_dir);
        // walker.into_iter().filter_entry( wj)
        walker
            .into_iter()
            .filter_entry(is_org_file)
            .map(|r| r.unwrap())
            .map(|de| de.path().to_path_buf())
            .collect()
    }
    /// Verify if a single DirEntry is an org file.
    /// It verifies if a DirEntry is both a file and if it is, if it's extension is ".org"
    fn is_org_file(entry: &DirEntry) -> bool {
        if entry.metadata().unwrap().is_file() {
            return entry
                .file_name()
                .to_str()
                .map(|s| s.ends_with(".org"))
                .unwrap_or(false);
        }
        false
    }
    /// Returns the content of the org files inside the org directory as a Vector of String.
    async fn read_org_files(org_dir: &str) -> Vec<String> {
        let org_entries = get_org_entries(org_dir);
        let mut string_files: Vec<String> = vec![];
        for entry in org_entries {
            if let Ok(file_string) = read_to_string(entry).await {
                string_files.push(file_string)
            }
        }
        string_files
    }
    /// Generate the TodoVec for a given file converted into a String.
    fn iterate_over_file(file: String) -> TodoVec {
        let todo_list: TodoVec = file
            .as_str()
            .par_lines()
            .filter(|l| Todo::filter(l))
            .map(Todo::parse_todo)
            .filter(|t| t.is_some())
            .map(|t| t.unwrap())
            .collect();
        todo_list
    }
    /// Generate all the todos for a fiven org_directory
    /// This function is the entry point for parsing the org directory and the org files
    pub async fn generate_todos(org_dir: &str) -> TodoVec {
        let files_content = read_org_files(org_dir).await;
        let todo_vec: Vec<Vec<Todo>> = files_content
            .into_par_iter()
            .map(iterate_over_file)
            .collect();
        todo_vec.into_iter().flatten().collect::<Vec<Todo>>() // Flatten the vector of vector into a TodoVec
    }

    /// The struct holding reference to a single todo.
    /// Its role is to parse a given line into an easy to manipulate todo item
    #[derive(Clone, Eq, PartialEq)]
    pub struct Todo {
        item: String,
        date: NaiveDateTime,
    }

    /// Our Todo list.
    /// A file has a single (possibly empty) TodoList
    /// We have as manu TodoVec objects as we have org files inside the org directory
    type TodoVec = Vec<Todo>;

    /// Verify if line contains a 'TODO' item and date and if so, generate a single Todo for a given line
    impl Todo {
        pub fn parse_todo(line: &str) -> Option<Todo> {
            let item: Vec<&str> = line.split("*TODO").collect();
            let item = String::from(item[1]); // Select the second item, following the "*TOD O"
            match Self::parse_date(line) {
                Ok(datetime) => Some(Todo {
                    item,
                    date: datetime,
                }),
                Err(_) => None,
            }
        }
        /// Verify if a line contains "*TODO" and ("DEALINE" or "SCHEDULE")
        pub fn filter(line: &str) -> bool {
            line.contains("*TODO") && (line.contains("DEADLINE") || line.contains("SCHEDULED"))
        }
        /// Find the date inside of a line (&str)
        //BUG: problem when there is another '<' inside the T O D O object
        //BUG: problem when there is a ' ' (blank space) before the date. Example: < 2023-05-18 ...>
        // This will print an error in case of failure, but won't panic otherwise
        fn parse_date(line: &str) -> Result<NaiveDateTime, ParseError> {
            let parse_from_str = NaiveDateTime::parse_from_str;
            let date_str = Self::find_date(line);
            let formated_with_date_and_time = parse_from_str(date_str, "%Y-%m-%d %a %H:%M"); // Formater. Example: 2023-09-05 Tue 10:06
            let formated_with_date = parse_from_str(date_str, "%Y-%m-%d %a"); // Formater. Example: 2023-09-05 Tue
            let formated_with_time = parse_from_str(date_str, "%Y-%m-%d %H:%M"); // Formater. Example: 2023-09-05 10:06
            let formated = parse_from_str(date_str, "%Y-%m-%d"); // Formater. Example: 2023-09-05
                                                                 // These conditionals will verify if the date is parsed for at leat one of the 3 parsers above
                                                                 // If not, it will return an error
            if formated_with_date_and_time.is_ok() {
                formated
            } else if formated_with_date.is_ok() {
                formated_with_date
            } else if formated_with_time.is_ok() {
                formated_with_time
            } else if formated.is_ok() {
                formated
            } else {
                println!("error:{}", formated_with_date_and_time.unwrap_err());
                formated_with_date_and_time
            }
        }
        fn find_date(line: &str) -> &str {
            let date_split: Vec<&str> = line.split('<').collect();
            let right_of_date = date_split[1]; // right side of "<"
            let date_str_split: Vec<&str> = right_of_date.split('>').collect();
            date_str_split[0] // left side of " " (blank space)
        }
    }
    impl fmt::Display for Todo {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let item = &self.item;
            let date = &self.date;
            write!(f, "{item},{date}")
        }
    }
    mod tests {
        #[test]
        fn test_finding_date() {
            let line0 = "*TODO this should be good <2023-08-08>"; //NOTE: Must change Sun to another day
            let line1 = "*TODO this should be good <2023-08-08 Sun 10:10>"; //NOTE: Must change Sun to another day
            let line2 = "*TODO this should be good too <2023-08-08 10:10>";
            assert_eq!(super::Todo::find_date(line0), "2023-08-08");
            assert_eq!(super::Todo::find_date(line1), "2023-08-08 Sun 10:10");
            assert_eq!(super::Todo::find_date(line2), "2023-08-08 10:10")
        }

        #[test]
        fn testing_parse_todo() {
            let line0 = "*TODO this should be good <2023-08-08>"; //NOTE: Must change Sun to another day
            let line1 = "*TODO this should be good <2023-08-08 Sun 10:10>";
            let line2 = "*TODO this should be good too <2023-08-08 10:10>";
            let line3 = "*TODO this should be no gucci <Sun 10:10>";
            let line4 = "**TODO this should be no gucci <10:10>";
            let good_lines = vec![line0, line1, line2];
            let bad_lines = vec![line3, line4];
            for lines in good_lines {
                let x = super::Todo::parse_date(lines);
                assert!(x.is_ok())
            }
            for lines in bad_lines {
                let x = super::Todo::parse_date(lines);
                assert!(x.is_err())
            }
        }
    }
}
#[tokio::main]
async fn main() {
    let org_dir = "/home/simon/org"; // Should be absolute path!
    let todo_vec = generate_todos(org_dir).await;
    println!("{}", todo_vec.len() ); //BUG: length of vector is 0
    for todo in todo_vec {
        println!("{todo}");
    }
}

#[cfg(test)]
mod tests {
    use super::parsing;
    use core::iter::zip;
    #[test]
    fn filtering_lines() {
        let lines = vec![
            "this lines contains *TODO",
            "*TODO: ranger ma chambre DEADLINE: <blabla>",
            "*TODO: ranger ma chambre SCHEDULED: <blabla>",
            " ranger ma chambre SCHEDULED: <blabla>",
        ];
        let right_answers = vec![false, true, true, false];
        for (line, answer) in zip(lines, right_answers) {
            assert_eq!(parsing::Todo::filter(line), answer);
        }
    }
    #[test]
    fn testing_parse_todo() {
        let line1 = "*TODO this should be good <2023-08-08 Mon 10:10>";
        let line2 = "*TODO this should be good too <2023-08-08 10:10>";
        let line3 = "*TODO this should be no gucci <Mon 10:10>";
        let line4 = "**TODO this should be no gucci <10:10>";
        let good_lines = vec![line1, line2];
        let bad_lines = vec![line3, line4];
        for lines in good_lines {
            let x = parsing::Todo::parse_todo(lines);
            println!("{}", lines);
            assert!(x.is_some(), "value of parsing: {}:", x.unwrap())
        }
        for lines in bad_lines {
            let x = parsing::Todo::parse_todo(lines);
            println!("{}", lines);
            assert!(x.is_none(), "value of parsing: {}:", x.unwrap())
        }
    }
}
