"""COCO-style detection dataset and the transforms applied to its samples."""

import torch
from torch.utils.data import Dataset
from torchvision import transforms as T


class CocoDetection(Dataset):
    """Image/annotation pairs, with the configured transform pipeline applied."""

    def __init__(self, root, split="train", transforms=None):
        self.root = root
        self.split = split
        self.transforms = transforms
        self.samples = []

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, index):
        image, target = self.samples[index]
        if self.transforms is not None:
            image = self.transforms(image)
        return image, torch.as_tensor(target)


def build_transforms(image_size=640, augment=True):
    """Resize and, for training, augment. Changing this moves mAP."""
    steps = [T.Resize((image_size, image_size)), T.ToTensor()]
    if augment:
        steps.insert(0, T.RandomHorizontalFlip(p=0.5))
    return T.Compose(steps)
