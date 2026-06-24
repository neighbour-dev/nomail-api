package webhook

type Registry struct {
	parsers map[string]Parser
}

func NewRegistry() *Registry {
	return &Registry{parsers: make(map[string]Parser)}
}

func (r *Registry) Register(parser Parser) {
	r.parsers[parser.Provider()] = parser
}

func (r *Registry) Get(provider string) (Parser, bool) {
	p, ok := r.parsers[provider]
	return p, ok
}
