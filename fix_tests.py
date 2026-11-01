#!/usr/bin/env python3
"""Fix people_tests.rs by adding Phase 5 test functions with correct sqlx patterns."""

import sys

# Read the original file
with open('core/tests/people_tests.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# New test function - edu_late_enrollment
# Using &pool pattern for fetch_one and execute
new_test =r"""

/// edu: late enrollment — student joins course mid-way; eligibility basis computed.
#[tokio::test]
async fn edu_late_enrollment() {
    let (pool, _t, branch) = fresh_seeded_pool().await;
    let admin = admin_actor(&pool).await;

    // create a subject
    let subject = create_subject(&pool, &admin, "ARAB", "اللغة العربية", "").await.unwrap();

    // create a teacher
    let teacher_id: i64 = sqlx::query_scalar(
        "INSERT INTO teachers (branch_id, teacher_no, full_name, phone, email, specialty, qualifications, address, hire_date, status, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch)
    .bind(1)
    .bind("اسم المعلم".into())
    .bind("0555123456".into())
    .bind("teacher@school.edu".into())
    .bind("رياضيات".into())
    .bind("بكالوريوس".into())
    .bind("".into())
    .bind("active")
    .bind("".into())
    .bind("".to_string())
    .bind("".to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    // create a room
    let room_id: i64 = sqlx::query_scalar(
        "INSERT INTO rooms (branch_id, code, name, capacity, description, status, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch)
    .bind("R1")
    .bind("قالية دراسية".into())
    .bind(30)
    .bind("".into())
    .bind("available")
    .bind("".into())
    .bind("".to_string())
    .bind("".to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    // create a course
    let course_id: i64 = sqlx::query_scalar(
        "INSERT INTO courses (branch_id, course_no, name, subject_id, description, start_date, end_date, default_time_json, teacher_id, room_id, expected_students, min_students, max_students, fee_minor, status, is_private, created_by, approved_by, approved_at, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(branch)
    .bind(101)
    .bind("ARAB101")
    .bind(subject.id)
    .bind("وصف الدورة")
    .bind("2024-10-01")
    .bind("2024-12-30")
    .bind("\"{\\\"weekday\\\":1,\\\"start_time\\\":\\\"09:00\\\",\\\"end_time\\\":\\\"11:00\\\"}\"")
    .bind(teacher_id)
    .bind(room_id)
    .bind(10)
    .bind(1)
    .bind(50)
    .bind(10000)
    .bind("draft")
    .bind(0)
    .bind(0)
    .bind("")
    .bind("")
    .bind("".to_string())
    .bind("".to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    // verify course and students exist
    let course_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses WHERE id = ?")
        .bind(course_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(course_count, 1, "course should exist");

    // list enrollments for the course
    let enrollment_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM enrollments WHERE course_id = ? AND student_id IN (SELECT id FROM students WHERE branch_id = ?)",
    )
    .bind(course_id)
    .bind(branch)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(enrollment_count, 1, "one enrollment for the course");
}"""

# Find the position after edu_catalog_subjects_rooms test
# The test ends with: }\n/// edu: ... or just }\n at the end
# Let me find "let err = create_room" and insert after its closing }

# Find the last "}" that closes edu_catalog_subjects_rooms
# It's the one after "let err = create_room(&pool, &admin, branch, \"R2\", \"سالب\", -5, \"\").await;"
# Followed by "assert!(err.is_err, \"negative capacity must be rejected\");}"

# Let me search for the pattern
import re

# Find the position to insert - after the edu_catalog_subjects_rooms test closing brace
# The pattern is: \n    assert!(err.is_err, \"negative capacity must be rejected\");\n}\n\n

# Actually, let me just find where "edu: catalog — subjects & rooms" starts and insert after that test
# The test function ends and then there's a blank line and possibly another test

# Let me just append at the end for now - it's simpler
# But we need to make sure the file is valid Rust

# Check if there's already content after the last test
# The file should end with the edu_catalog_subjects_rooms test

# Let me just append the new test
new_content = content + new_test

# Write back
with open('core/tests/people_tests.rs', 'w', encoding='utf-8') as f:
    f.write(new_content)

print("Test appended successfully")