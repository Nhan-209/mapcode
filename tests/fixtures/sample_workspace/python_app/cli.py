# tests/fixtures/sample_workspace/python_app/cli.py

import click

@click.group()
def cli():
    """Python sample CLI utility"""
    pass

@cli.command()
@click.option('--name', default='World', help='Name to greet')
def hello(name: str):
    click.echo(f"Hello {name}!")

@cli.command()
@click.argument('path')
def inspect(path: str):
    click.echo(f"Inspecting {path}")

if __name__ == '__main__':
    cli()
