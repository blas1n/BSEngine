# DownloadExternal.py
from threading import Thread
from pathlib import Path
from requests import get
import tarfile
import shutil
import os

def make_abs_path(*names):
    ret = Path.cwd().parent / 'External'
    for name in names:
        ret /= name
    return ret

def init_folder():
    if os.path.exists(make_abs_path()):
        shutil.rmtree(make_abs_path())
    os.mkdir(make_abs_path())

def download(name, url):
    print(f'Start downloading {name}...')
    path = make_abs_path(Path(url).name)
    path.write_bytes(get(url).content)

    print(f'Extracting {name}...')
    with tarfile.open(path) as file:
        file.extractall(make_abs_path())

    path.unlink()
    folder_path = Path(str(path)[:-7])
    folder_path.rename(make_abs_path(name))
   
    print(f'{name} download complete.')

if __name__ == '__main__':
    init_folder()
    threads = []
    
    external_path = Path.cwd().parent / 'ExternalPath.txt'
    for line in external_path.read_text().splitlines():
        name, url = line.split(': ')
        thread = Thread(target=download, args=(name, url))
        thread.start()
        threads.append(thread)
    
    for thread in threads:
        thread.join()