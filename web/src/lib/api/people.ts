/// People IPC surface — students/teachers/subjects/rooms (Phase 4).
import { invoke } from '$lib/api';

export type Student = {
  id: number;
  branch_id: number;
  student_no: number;
  full_name: string;
  national_id: string;
  birth_date: string;
  gender: string | null;
  phone: string;
  email: string;
  address: string;
  notes: string;
  status: 'active' | 'frozen' | 'archived';
  registered_at: string;
};

export type Teacher = {
  id: number;
  branch_id: number;
  teacher_no: number;
  full_name: string;
  phone: string;
  email: string;
  specialty: string;
  qualifications: string;
  address: string;
  hire_date: string;
  status: 'active' | 'inactive' | 'archived';
  notes: string;
};

export type Subject = {
  id: number;
  code: string;
  name: string;
  description: string;
  is_active: boolean;
};

export type Room = {
  id: number;
  branch_id: number;
  code: string;
  name: string;
  capacity: number;
  description: string;
  status: string;
};

export const peopleApi = {
  students: {
    create: (
      token: string,
      data: {
        branch_id: number;
        full_name: string;
        national_id: string;
        birth_date: string;
        gender: string | null;
        phone: string;
        email: string;
        address: string;
        notes: string;
      }
    ) => invoke<Student>('students_create', { token, ...data }),

    list: (
      token: string,
      branchId: number,
      status?: string,
      search?: string,
      limit = 100
    ) =>
      invoke<Student[]>('students_list', {
        token,
        branch_id: branchId,
        status,
        search,
        limit
      }),

    setStatus: (token: string, id: number, status: string, reason: string) =>
      invoke<Student>('students_set_status', { token, id, status, reason })
  },
  teachers: {
    list: (token: string, branchId: number, status?: string, search?: string) =>
      invoke<Teacher[]>('teachers_list', {
        token,
        branch_id: branchId,
        status,
        search
      })
  },
  subjects: {
    list: (token: string, activeOnly = true) =>
      invoke<Subject[]>('subjects_list', { token, active_only: activeOnly })
  },
  rooms: {
    list: (token: string, branchId: number) =>
      invoke<Room[]>('rooms_list', { token, branch_id: branchId })
  }
};
