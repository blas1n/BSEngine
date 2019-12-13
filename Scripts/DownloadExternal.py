# DownloadExternal.py
from requests import get
import tarfile
import os

def make_abs_dir(name):
    abs_dir = os.path.join(os.path.dirname(os.getcwd()), 'External')
    return os.path.join(abs_dir, name)

def download(url, file_dir):
    path, name = os.path.split(file_dir)
    name = os.path.splitext(os.path.splitext(name)[0])[0]

    print(f'Downloading {name}...')
    with open(file_dir, "wb") as file:
        response = get(url)
        file.write(response.content)

    print(f'Extracting {name}...')
    with tarfile.open(file_dir) as file:
        file.extractall(path)

    os.remove(file_dir)
    print(f'{name} download complete.')

if __name__ == '__main__':
    with open(make_abs_dir('ExternalPath'), 'rt') as file:
        for path in file.readlines():
            abs_dir = make_abs_dir(os.path.basename(path))
            download(path, abs_dir)
