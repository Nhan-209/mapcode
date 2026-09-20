// tests/fixtures/sample_workspace/go_app/models.go

package main

type BaseRecord struct {
	ID        int64  `json:"id"`
	CreatedAt string `json:"created_at"`
}

func (b *BaseRecord) GetID() int64 {
	return b.ID
}

type Customer struct {
	BaseRecord
	Name  string `json:"name"`
	Email string `json:"email"`
}

func (c *Customer) FullInfo() string {
	return c.Name + " <" + c.Email + ">"
}
