//! People & catalog domain (Phase 4): students, teachers, subjects, rooms.
//! Services take an `Actor`, enforce permissions in Rust (ADR-008), and
//! write audit rows for every mutation.

pub mod context;
pub mod guardians;
pub mod student_credit;
pub mod students;
pub mod teachers;

pub use context::{log_audit, snapshot, Actor};
pub use guardians::{apply_credit_to_enrollment, create_guardian, link_guardian_to_student, refund_to_credit, Guardian};
pub use student_credit::{apply_credit_to_enrollment as apply_credit, refund_to_credit as refund_credit};
pub use students::{edit_student, list_students, set_student_status, Student, StudentFilter, NewStudent, StudentPatch};
pub use teachers::{create_teacher, linked_teacher_id, list_teachers, scope_for_teacher, teacher_id_for_user, Teacher, TeacherFilter, NewTeacher};
