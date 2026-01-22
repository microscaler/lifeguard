-- schema.sql

CREATE TABLE owners (
                        id SERIAL PRIMARY KEY,
                        name TEXT NOT NULL,
                        phone TEXT
);

CREATE TABLE pets (
                      id SERIAL PRIMARY KEY,
                      name TEXT NOT NULL,
                      species TEXT NOT NULL,
                      owner_id INTEGER REFERENCES owners(id)
);

CREATE TABLE appointments (
                              id SERIAL PRIMARY KEY,
                              pet_id INTEGER REFERENCES pets(id),
                              date TIMESTAMP NOT NULL,
                              notes TEXT
);
