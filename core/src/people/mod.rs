//! People & catalog domain (Phase 4): students, teachers, subjects, rooms.
//! Services take an `Actor`, enforce permissions in Rust (ADR-008), and
//! write audit rows for every mutation.

pub mod context;
pub mod guardians;
pub mod students;
pub mod teachers;

pub use context::{log_audit, snapshot, Actor};
pub use guardians::{apply_credit_to_enrollment, create_guardian, link_guardian_to_student, Guardian};
pub use students::{edit_student, list_students, set_student_status, Student, StudentFilter, NewStudent, StudentPatch};
pub use teachers::{create_teacher, list_teachers, Teacher, TeacherFilter, NewTeacher};
