# DownloadExternal.py
import tarfile
import sys
import os

try:
    from requests import get
except ImportError:
    os.system("pip install "+ request)

def make_dir(name):
    dir = os.path.join(os.path.dirname(os.getcwd()), 'External')
    return os.path.join(dir, name)

def download(url, dir):
    path, name = os.path.split(dir)
    name = os.path.splitext(os.path.splitext(name)[0])[0]

    print(f'Downloading {name}...')
    with open(dir, "wb") as file:
        response = get(url)
        file.write(response.content)

    print(f'Extracting {name}...')
    with tarfile.open(dir) as file:
        file.extractall(path)

    os.remove(dir)
    print(f'{name} download complete.')

if __name__ == '__main__':
    with open(make_dir('ExternalPath'), 'rt') as file:
        for path in file.readlines():
            dir = make_dir(os.path.basename(path))
            download(path, dir)