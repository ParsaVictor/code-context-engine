"""Training entry point: dataset -> transforms -> model -> loss -> checkpoint."""

import torch
from torch.utils.data import DataLoader

from data.dataset import CocoDetection, build_transforms
from models.detector import Detector, detection_loss


def train_one_epoch(model, loader, optimizer):
    model.train()
    running = 0.0
    for images, targets in loader:
        optimizer.zero_grad()
        predictions = model(images)
        loss = detection_loss(predictions, targets)
        loss.backward()
        optimizer.step()
        running += float(loss)
    return running


def main(epochs=2, image_size=640, batch_size=16, lr=0.01):
    pipeline = build_transforms(image_size=image_size, augment=True)
    dataset = CocoDetection("data/coco", split="train", transforms=pipeline)
    loader = DataLoader(dataset, batch_size=batch_size, shuffle=True)

    model = Detector(num_classes=80)
    optimizer = torch.optim.SGD(model.parameters(), lr=lr)

    for epoch in range(epochs):
        train_one_epoch(model, loader, optimizer)
        torch.save(model.state_dict(), "runs/last.pt")
