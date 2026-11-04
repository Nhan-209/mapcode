package main

type Server struct {
	port int
}

func (s *Server) Close() {
	// custom close method
}

func (s *Server) Run() {
	ch := make(chan int)
	close(ch) // Go builtin close, NOT server method
}

func main() {
	srv := new(Server) // Go builtin new
	srv.Run()
	srv.Close()
}
