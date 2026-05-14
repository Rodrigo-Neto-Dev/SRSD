package com.srsd.gallery;

import java.util.Objects;

final class PersonKey {
    final boolean employee;
    final String name;

    PersonKey(boolean employee, String name) {
        this.employee = employee;
        this.name = Objects.requireNonNull(name);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof PersonKey personKey)) return false;
        return employee == personKey.employee && name.equals(personKey.name);
    }

    @Override
    public int hashCode() {
        return Objects.hash(employee, name);
    }
}
