//! Education domain (Phase 5): courses, schedules, sessions, attendance,
//! enrollments, agreements.

pub mod agreements;
pub mod attendance;
pub mod courses;
pub mod enrollments;
pub mod payment_plans;
pub mod sessions;

pub use agreements::{active_agreement_for_course, amend_agreement, approve_agreement, create_agreement_draft, Agreement};
pub use attendance::{record_student_attendance, record_teacher_attendance, StudentAttendance, TeacherAttendance};
pub use courses::{check_maker_checker, create_course, get_course, list_courses, set_course_status, Course, NewCourse};
pub use enrollments::{enroll, list_enrollments, set_enrollment_status, Enrollment};
pub use payment_plans::{create_payment_plan, list_payment_plan, record_installment_payment, PaymentPlan, Installment};
pub use sessions::{add_schedule, create_session, link_makeup_original, list_sessions, set_session_status, CourseSchedule, Session};
